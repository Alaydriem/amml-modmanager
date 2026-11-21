use amm_ffi::types::AmmHostFrameInput;
use amml_api::{ComRef, DrawSurface};
use windows::Win32::Graphics::Direct3D11::ID3D11Device;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

use crate::draw_pass::DrawPass;
use crate::draw_report::DrawReport;
use crate::held_frame::HeldFrame;
use crate::layer_renderer::LayerRenderer;
use crate::overlay_error::OverlayError;
use crate::overlay_host::OverlayHost;

/// Draws the manager's host frames on the loader's `DrawSurface`, inside `FramePresent` listeners, on the render
/// thread. Its device objects (the layer pipeline, the layer textures and the game picture) are made on
/// `surface.device()` at the first call of a device epoch and live until `surface_lost`, or until a surface of
/// another epoch comes. It keeps one reference to the device of that epoch, never one to the back buffer or the
/// target: each frame's context and target are borrowed for the call only, and the target is unbound before the
/// call returns. It makes no DXGI or window call.
///
/// A present is drawn in one call (`draw`) or in two: `draw_mods` asks the host for the frame and draws the mod
/// layers (every layer before the first manager layer), and `draw_manager`, later in the same present, draws the
/// manager's panel and toasts of that frame. Between the two the loader takes its `WithOverlays` capture.
pub struct SurfaceRenderer {
    host: &'static dyn OverlayHost,
    // the device of `epoch`, with its own reference; the objects of `renderer` live on it
    device: Option<ID3D11Device>,
    renderer: Option<LayerRenderer>,
    // the device epoch the objects were made for, or failed to be made for
    epoch: Option<u64>,
    // the objects could not be made on the device of `epoch`; tried again only for another epoch
    failed: bool,
    // a failed game picture copy was logged for this epoch
    picture_logged: bool,
    // the frame of this present whose manager layers wait for `draw_manager`
    held: Option<HeldFrame>,
}

impl SurfaceRenderer {
    pub fn new(host: &'static dyn OverlayHost) -> Self {
        Self { host, device: None, renderer: None, epoch: None, failed: false, picture_logged: false, held: None }
    }

    /// Draws every layer of the host frame for `input` into `surface.target()`: `draw_mods`, then `draw_manager`.
    pub fn draw(&mut self, surface: &DrawSurface<'_>, input: &AmmHostFrameInput) -> DrawReport {
        let mods = self.draw_mods(surface, input);
        mods.and(self.draw_manager(surface))
    }

    /// Asks the host for the frame of `input` and draws its mod layers into `surface.target()`; its manager layers
    /// wait for `draw_manager`. Skips the frame (`OverlayHost::frame_skipped`) before the game is ready, when the
    /// device objects cannot be made, and when the host has no frame. A frame that asks for the game picture
    /// refreshes it first; a failed copy leaves the picture's commands undrawn.
    pub fn draw_mods(&mut self, surface: &DrawSurface<'_>, input: &AmmHostFrameInput) -> DrawReport {
        self.held = None;
        let host = self.host;
        if !host.game_ready() || !self.ensure(surface) {
            host.frame_skipped();
            return DrawReport::default();
        }
        let Some(frame) = host.frame(input) else {
            host.frame_skipped();
            return DrawReport::default();
        };
        let frame = *frame;
        if frame.game_picture {
            self.refresh_picture_logged(surface);
        }
        // SAFETY: OverlayHost promises the frame's pointers stay valid until its next frame, after this call
        let manager_from = unsafe { LayerRenderer::mod_layer_end(&frame) };
        // SAFETY: as above
        let report = self.pass(surface, |pass| unsafe { pass.first(&frame, 0..manager_from) });
        self.held = Some(HeldFrame { frame, epoch: surface.device_epoch(), manager_from });
        report
    }

    /// Draws the manager layers of the frame the last `draw_mods` of this present asked for. Nothing when that
    /// call drew no frame or the surface is of another device epoch.
    pub fn draw_manager(&mut self, surface: &DrawSurface<'_>) -> DrawReport {
        let Some(held) = self.held.take() else {
            return DrawReport::default();
        };
        if held.epoch != surface.device_epoch() || self.epoch != Some(held.epoch) {
            return DrawReport::default();
        }
        let layers = held.manager_from..held.frame.layer_count as usize;
        // SAFETY: the host has not been asked for another frame since `draw_mods` took this one, so its pointers are
        // still valid (the OverlayHost contract)
        self.pass(surface, |pass| unsafe { pass.rest(&held.frame, layers) })
    }

    /// Releases every object made on the device and the device reference, then tells the host its textures are
    /// lost. The host hears it once per loss: a call with nothing held tells it nothing.
    pub fn surface_lost(&mut self) {
        let held = self.live_objects();
        self.held = None;
        self.renderer = None;
        self.device = None;
        self.epoch = None;
        self.failed = false;
        self.picture_logged = false;
        if held > 0 {
            curia::info!("overlay device objects released", { objects: held });
            self.host.textures_lost();
        }
    }

    /// Copies the back buffer of `surface`, as drawn so far, into the game picture the layers draw. The picture is
    /// made, or made again, for the surface's size and format. On an error the picture is not drawn until a later
    /// copy succeeds.
    pub fn refresh_game_picture(&mut self, surface: &DrawSurface<'_>) -> Result<(), OverlayError> {
        if !self.ensure(surface) {
            return Err(OverlayError::NoDevice);
        }
        let (Some(device), Some(renderer)) = (self.device.as_ref(), self.renderer.as_mut()) else {
            return Err(OverlayError::NoDevice);
        };
        let size = surface.size();
        let format = DXGI_FORMAT(surface.format() as i32);
        let picture = renderer.game_picture_mut();
        let copied = match picture.prepare(device, size.width, size.height, format) {
            Ok(texture) => surface.copy_back_buffer(ComRef::from_interface(texture)),
            Err(error) => {
                picture.set_copied(false);
                return Err(error);
            }
        };
        picture.set_copied(copied.is_ok());
        copied.map_err(|source| OverlayError::Copy { source })
    }

    /// The device objects held, the device reference among them; 0 after `surface_lost`.
    pub fn live_objects(&self) -> usize {
        usize::from(self.device.is_some()) + self.renderer.as_ref().map_or(0, LayerRenderer::object_count)
    }

    // runs one draw pass on the surface's context and target, borrowed for the call
    fn pass(&mut self, surface: &DrawSurface<'_>, run: impl FnOnce(DrawPass<'_>) -> DrawReport) -> DrawReport {
        let Some(renderer) = self.renderer.as_mut() else {
            return DrawReport::default();
        };
        let size = surface.size();
        let drawn = surface.context().with(|ctx| {
            surface.target().with(|target| run(DrawPass { renderer, ctx, target, size: (size.width, size.height) }))
        });
        drawn.flatten().unwrap_or_default()
    }

    // the objects of the surface's epoch: kept for the same epoch, made for a new one after the old ones are dropped
    fn ensure(&mut self, surface: &DrawSurface<'_>) -> bool {
        let epoch = surface.device_epoch();
        if self.epoch == Some(epoch) {
            return !self.failed;
        }
        if self.epoch.is_some() {
            curia::info!("overlay sees a new device epoch without a surface loss, dropping the old objects", {
                epoch: epoch
            });
            self.surface_lost();
        }
        self.epoch = Some(epoch);
        let Some(device) = surface.device().with(ID3D11Device::clone) else {
            self.failed = true;
            return false;
        };
        match LayerRenderer::new(&device) {
            Ok(renderer) => {
                let size = surface.size();
                curia::info!("overlay device objects made", {
                    epoch: epoch,
                    width: size.width,
                    height: size.height,
                    format: surface.format(),
                    wrapped: surface.wrapped()
                });
                self.device = Some(device);
                self.renderer = Some(renderer);
                true
            }
            Err(error) => {
                self.failed = true;
                curia::error!("overlay layer renderer could not start, no layers are drawn on this device", {
                    epoch: epoch,
                    error: error.to_string()
                });
                false
            }
        }
    }

    fn refresh_picture_logged(&mut self, surface: &DrawSurface<'_>) {
        let Err(error) = self.refresh_game_picture(surface) else {
            return;
        };
        if !self.picture_logged {
            self.picture_logged = true;
            curia::warn!("game picture copy failed, layers that draw it show nothing", { error: error.to_string() });
        }
    }
}
