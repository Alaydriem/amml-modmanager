use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use amm_ffi::types::AmmHostFrameInput;
use amm_overlay::{OverlayHost, PresentPace, SurfaceRenderer};
use amml_api::amml_abi::{FramePresent, FramePresented, RenderDeviceReset, SurfaceLost};
use amml_api::{ApiError, Ctx, DrawSurface, FramePresentExt, HookOrder, Logger, Window};

use super::{ManagerClock, ManagerListeners, ManagerOverlayHost, RenderSizeControl};

/// The manager's drawing, on the render thread. Two `FramePresent` listeners share one `SurfaceRenderer`: in
/// `HookOrder::NORMAL` it asks the host for the frame and draws the mod layers, in `HookOrder::LAST` it draws the
/// panel and the toasts of that frame. The loader takes its `WithOverlays` capture between the two, so a screenshot
/// with overlays holds the mod layers and not the manager's own UI. A present with no surface draws nothing and
/// counts as skipped. `SurfaceLost` releases every device object (the consumers get `textures_lost` once). The
/// first frame of each bind logs `overlay bound`. `FramePresented` feeds the log-only `PresentPace`. In a steady
/// frame the listeners do not allocate.
pub struct PresentListener {
    renderer: Mutex<SurfaceRenderer>,
    host: &'static ManagerOverlayHost,
    window: Window,
    clock: ManagerClock,
    log: Logger,
    size: Arc<RenderSizeControl>,
    pace: PresentPace,
    // the sync interval (high half) and the flags (low half) of the last `FramePresent`
    present_args: AtomicU64,
    // the surface of this bind was logged; cleared at `SurfaceLost`
    bound_logged: AtomicBool,
}

impl PresentListener {
    /// The band of the mod layers: before the loader's `WithOverlays` capture.
    pub const MODS: HookOrder = HookOrder::NORMAL;
    /// The band of the panel and the toasts: after every other listener and the capture.
    pub const PANEL: HookOrder = HookOrder::LAST;
    const DEFAULT_DPI: f32 = 96.0;

    pub fn new(
        ctx: &Ctx,
        host: &'static ManagerOverlayHost,
        clock: ManagerClock,
        size: Arc<RenderSizeControl>,
    ) -> Self {
        Self {
            renderer: Mutex::new(SurfaceRenderer::new(host)),
            host,
            window: ctx.window(),
            clock,
            log: ctx.log(),
            size,
            pace: PresentPace::new(),
            present_args: AtomicU64::new(0),
            bound_logged: AtomicBool::new(false),
        }
    }

    /// Adds the present, loss and pace listeners to `listeners`. `FramePresent` and `SurfaceLost` are required;
    /// `FramePresented` and `RenderDeviceReset` are skipped when the loader cannot serve them.
    pub fn listen(self: &Arc<Self>, ctx: &Ctx, listeners: &mut ManagerListeners) -> Result<(), ApiError> {
        let events = ctx.events();
        let mods = Arc::clone(self);
        listeners.required(events.listen::<FramePresent>(Self::MODS, move |event, _| mods.on_present_mods(event)))?;
        let panel = Arc::clone(self);
        listeners
            .required(events.listen::<FramePresent>(Self::PANEL, move |event, _| panel.on_present_panel(event)))?;
        let lost = Arc::clone(self);
        listeners.required(events.listen::<SurfaceLost>(HookOrder::NORMAL, move |_, _| lost.on_surface_lost()))?;
        let presented = Arc::clone(self);
        listeners.optional(
            ctx,
            events.listen::<FramePresented>(HookOrder::NORMAL, move |event, _| presented.on_presented(event)),
        )?;
        let reset = Arc::clone(self);
        listeners.optional(
            ctx,
            events.listen::<RenderDeviceReset>(HookOrder::NORMAL, move |event, _| reset.on_device_reset(event)),
        )?;
        Ok(())
    }

    /// `FramePresent` in `MODS`: the host frame, its mod layers, then its size message to the game window.
    pub fn on_present_mods(&self, event: &FramePresent) {
        let args = (u64::from(event.sync_interval) << 32) | u64::from(event.flags);
        self.present_args.store(args, Ordering::Relaxed);
        let Some(surface) = event.surface() else {
            self.host.frame_skipped();
            return;
        };
        self.log_bound(&surface);
        let input = self.frame_input(&surface);
        self.lock().draw_mods(&surface, &input);
        if let Some(message) = self.host.take_size_message() {
            self.size.send(message);
        }
    }

    /// `FramePresent` in `PANEL`: the panel and the toasts of the frame `MODS` drew.
    pub fn on_present_panel(&self, event: &FramePresent) {
        if let Some(surface) = event.surface() {
            self.lock().draw_manager(&surface);
        }
    }

    /// `SurfaceLost`: every object made on the device goes now; the next surface logs its bind again.
    pub fn on_surface_lost(&self) {
        self.lock().surface_lost();
        self.bound_logged.store(false, Ordering::Relaxed);
    }

    /// `FramePresented`: the present's result and time, with the arguments of its `FramePresent`, for the pace log.
    pub fn on_presented(&self, event: &FramePresented) {
        let args = self.present_args.load(Ordering::Relaxed);
        self.pace.set_forced(self.size.forced().is_some());
        self.pace.observe(
            (args >> 32) as u32,
            args as u32,
            event.result,
            u64::from(event.cpu_us),
            self.window.is_foreground(),
        );
    }

    /// `RenderDeviceReset`: the loader made a new device.
    pub fn on_device_reset(&self, event: &RenderDeviceReset) {
        self.log.info("render device reset", &[("epoch", &event.device_epoch.to_string())]);
    }

    /// The frame input of `surface`: its size and format, the window's client size and scale, and the clock.
    pub fn frame_input(&self, surface: &DrawSurface<'_>) -> AmmHostFrameInput {
        let size = surface.size();
        let client = self.window.client_size().unwrap_or_default();
        let dpi = self.window.dpi().filter(|dpi| *dpi != 0);
        AmmHostFrameInput {
            width: size.width,
            height: size.height,
            pixels_per_point: dpi.map_or(1.0, |dpi| dpi as f32 / Self::DEFAULT_DPI),
            time_secs: self.clock.now_secs(),
            now_ms: self.clock.now_ms(),
            menu_screen: 0,
            format: surface.format(),
            client_width: client.width,
            client_height: client.height,
        }
    }

    fn log_bound(&self, surface: &DrawSurface<'_>) {
        if self.bound_logged.swap(true, Ordering::Relaxed) {
            return;
        }
        let size = surface.size();
        let (width, height) = (size.width.to_string(), size.height.to_string());
        let wrapped = if surface.wrapped() { "true" } else { "false" };
        self.log.info("overlay bound", &[("width", &width), ("height", &height), ("wrapped", wrapped)]);
    }

    // a panic in a listener is caught by the loader, so a poisoned lock still holds a renderer worth using
    fn lock(&self) -> MutexGuard<'_, SurfaceRenderer> {
        self.renderer.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
