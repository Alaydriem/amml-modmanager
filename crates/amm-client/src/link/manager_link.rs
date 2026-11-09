use std::ffi::c_void;
use std::io::Read;
use std::path::Path;

use amm_abi::convert::pointer_op::PointerOpCodec;
use amm_abi::types::{
    AmmCallbacks, AmmCapability, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmScreenshotLayers, AmmStatus,
};
use amm_core::errors::LinkError;
use amm_core::services::icon::IconDecoder;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::input::PointerOp;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::mesh::GamePicture;
use amm_core::structs::render::{RenderSize, RenderSizeState};
use amm_core::traits::AbiSized;
use curia::{Dispatcher, InstallError, Level, Logger, TracingBridge};
use serde::de::DeserializeOwned;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[cfg(feature = "egui")]
use super::FrameHandler;
use super::{ApiTable, CaptureStream, EventDrain, ForwardSink};
#[cfg(feature = "egui")]
use crate::bridge::{EguiFrame, FrameTrampoline, LocaleSlot};

/// A mod's registration with the manager. A link made with a frame handler owns the boxed frame state the
/// manager's callbacks use; a link without a frame owns no frame state. Drop unregisters, then frees the frame
/// state when there is one.
pub struct ManagerLink {
    table: ApiTable,
    handle: AmmHandle,
    // The frame state the callbacks take as `user`, null for a link without a frame
    user: *mut c_void,
    // Frees `user`; `None` for a link without a frame
    free: Option<unsafe fn(*mut c_void)>,
    drain: EventDrain,
    sink: ForwardSink,
    // The localizer the frame's layers read; a link without a frame has a slot no pass reads
    #[cfg(feature = "egui")]
    locale: LocaleSlot,
}

// SAFETY: the frame state behind `user`, when there is one, holds an `H: Send` and is reached only by the
// manager's render callbacks and by `Drop`, which runs after unregister; the rest of the link is plain data and a
// shared table
unsafe impl Send for ManagerLink {}

impl ManagerLink {
    // The manager filters by its per-mod level, so the mod side forwards every level.
    const LOG_LEVEL: Level = Level::Trace;
    // Room for the longest mod id the descriptor checker accepts, so one call is enough
    const HOLDER_ID_BYTES: usize = 64;
    // Room for a locale id such as `pt_BR` or `zh_Hant_TW`, so one call is enough
    const LOCALE_BYTES: usize = 16;

    /// Registers the mod from its descriptor text, with `handler` as its egui pass, and installs Curia in this
    /// DLL with a sink that forwards to the manager. Curia already being installed counts as success.
    /// `config_dir` is the mod's own AMML config folder, an absolute path; the manager keeps the mod's
    /// `config.hcl` there. A path that is not UTF-8 is `InvalidArgument`.
    #[cfg(feature = "egui")]
    pub fn new<H: FrameHandler + Send + 'static>(
        table: ApiTable,
        descriptor_hcl: &'static str,
        config_dir: &Path,
        handler: H,
    ) -> Result<ManagerLink, LinkError> {
        let frame = EguiFrame::new(handler);
        let locale = frame.locale().clone();
        let user = FrameTrampoline::<H>::into_user(frame);
        let callbacks = FrameTrampoline::<H>::callbacks(user);
        let mut link = Self::register(table, descriptor_hcl, config_dir, callbacks, Some(FrameTrampoline::<H>::free))?;
        link.locale = locale;
        Ok(link)
    }

    /// The localizer the mod's widgets resolve their text with, from the next pass on. Build it from `locale` at
    /// start and again on `ModEvent::LocaleChanged`; it lasts when the host rebuilds the egui contexts.
    #[cfg(feature = "egui")]
    pub fn set_localizer(&self, localizer: std::sync::Arc<amml_i18n::localize::Localizer>) {
        self.locale.set(localizer);
    }

    /// The localizer the mod's layers draw with now: English until `set_localizer`.
    #[cfg(feature = "egui")]
    pub fn localizer(&self) -> std::sync::Arc<amml_i18n::localize::Localizer> {
        self.locale.get()
    }

    /// Registers the mod from its descriptor text with no frame and no textures-lost callback, for a mod that
    /// draws nothing through the manager. Logging, `config_dir` and errors are as for `new`. Drop unregisters and
    /// frees nothing.
    pub fn without_frame(
        table: ApiTable,
        descriptor_hcl: &'static str,
        config_dir: &Path,
    ) -> Result<ManagerLink, LinkError> {
        let callbacks = AmmCallbacks::blank();
        Self::register(table, descriptor_hcl, config_dir, callbacks, None)
    }

    // Registers with `callbacks` and installs logging. On any error the manager holds no copy of
    // `callbacks.user`, so `free`, when given, frees it here once.
    fn register(
        table: ApiTable,
        descriptor_hcl: &'static str,
        config_dir: &Path,
        callbacks: AmmCallbacks,
        free: Option<unsafe fn(*mut c_void)>,
    ) -> Result<ManagerLink, LinkError> {
        let handle = match Self::register_handle(&table, descriptor_hcl, config_dir, &callbacks) {
            Ok(handle) => handle,
            Err(error) => {
                if let Some(free) = free {
                    // SAFETY: the manager refused the mod or was never called, so it holds no copy of user,
                    // which is freed here once
                    unsafe { free(callbacks.user) };
                }
                return Err(error);
            }
        };
        let sink = ForwardSink::new(table, handle, Self::LOG_LEVEL);
        Self::install_logging(&sink);
        Ok(Self {
            table,
            handle,
            user: callbacks.user,
            free,
            drain: EventDrain::new(),
            sink,
            #[cfg(feature = "egui")]
            locale: LocaleSlot::new(),
        })
    }

    fn register_handle(
        table: &ApiTable,
        descriptor_hcl: &'static str,
        config_dir: &Path,
        callbacks: &AmmCallbacks,
    ) -> Result<AmmHandle, LinkError> {
        let Some(register) = table.api().register else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let Some(config_dir) = config_dir.to_str() else {
            return Err(LinkError::from(AmmStatus::InvalidArgument));
        };
        let mut handle = AmmHandle { slot: u16::MAX, generation: u16::MAX };
        // SAFETY: the descriptor text, the config folder text, the callbacks and the handle all live for the call;
        // the manager copies the callbacks and writes the handle only on success
        let status = unsafe {
            register(
                descriptor_hcl.as_ptr(),
                descriptor_hcl.len(),
                config_dir.as_ptr(),
                config_dir.len(),
                callbacks,
                &mut handle,
            )
        };
        match status {
            AmmStatus::Ok => Ok(handle),
            other => Err(LinkError::from(other)),
        }
    }

    /// A forwarding sink for this mod's log events. It sends through the same handle as the link, and stops when
    /// the link is dropped.
    pub fn sink(&self) -> ForwardSink {
        self.sink.clone()
    }

    /// Asks for a lease on `capability`.
    pub fn acquire(&self, capability: Capability) -> Result<AcquireOutcome, LinkError> {
        let Some(acquire) = self.table.api().acquire else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: acquire takes plain values; the handle is this link's until drop
        let result = unsafe { acquire(self.handle, AmmCapability::from(capability) as u8) };
        Ok(AcquireOutcome::from(result))
    }

    /// Gives a lease back.
    pub fn release(&self, capability: Capability) {
        if let Some(release) = self.table.api().release {
            // SAFETY: release takes plain values; the handle is this link's until drop
            unsafe { release(self.handle, AmmCapability::from(capability) as u8) };
        }
    }

    /// The language code the person picked in the manager: empty for English, `en_XA` for the pseudo-locale, or a
    /// locale such as `ru`. Map it with `LocaleSetting::from_code`. After this call the mod gets
    /// `ModEvent::LocaleChanged` when the code changes. `None` on a manager older than ABI 1.5, which has no
    /// language setting: show English.
    pub fn locale(&self) -> Option<String> {
        let locale = self.table.api().locale?;
        let mut buffer = [0u8; Self::LOCALE_BYTES];
        // SAFETY: the buffer has room for its length in bytes for the call; the handle is this link's until drop
        let len = unsafe { locale(self.handle, buffer.as_mut_ptr(), buffer.len()) } as usize;
        if len <= buffer.len() {
            return std::str::from_utf8(&buffer[..len]).ok().map(str::to_owned);
        }
        let mut long = vec![0u8; len];
        // SAFETY: the buffer has room for its length in bytes for the call; the handle is this link's until drop
        let again = unsafe { locale(self.handle, long.as_mut_ptr(), long.len()) } as usize;
        if again > long.len() {
            return None;
        }
        std::str::from_utf8(&long[..again]).ok().map(str::to_owned)
    }

    /// The id of the mod that holds `capability` now, or `None` when no mod holds it.
    pub fn holder(&self, capability: Capability) -> Option<String> {
        let holder = self.table.api().holder?;
        let capability = AmmCapability::from(capability) as u8;
        let mut buffer = [0u8; Self::HOLDER_ID_BYTES];
        // SAFETY: the buffer has room for its length in bytes for the call
        let len = unsafe { holder(capability, buffer.as_mut_ptr(), buffer.len()) } as usize;
        if len <= buffer.len() {
            return Self::id_text(&buffer[..len]);
        }
        let mut long = vec![0u8; len];
        // SAFETY: the buffer has room for its length in bytes for the call
        let again = unsafe { holder(capability, long.as_mut_ptr(), long.len()) } as usize;
        if again > long.len() {
            return None;
        }
        Self::id_text(&long[..again])
    }

    /// The events queued for this mod, in order. The iterator reads a reused buffer of `EventDrain::CAPACITY`
    /// events; call again to drain the rest.
    pub fn events(&mut self) -> impl Iterator<Item = ModEvent> + '_ {
        self.drain.poll(&self.table, self.handle)
    }

    /// The mod's checked `config.hcl` parsed into its own type.
    pub fn config<T: DeserializeOwned>(&self) -> Result<T, LinkError> {
        let Some(config_document) = self.table.api().config_document else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let mut ptr = std::ptr::null();
        let mut len = 0;
        // SAFETY: both out pointers live for the call; the handle is this link's until drop
        let status = unsafe { config_document(self.handle, &mut ptr, &mut len) };
        if status != AmmStatus::Ok {
            return Err(LinkError::from(status));
        }
        let bytes: &[u8] = if ptr.is_null() || len == 0 {
            &[]
        } else {
            // SAFETY: the manager owns the text and keeps it valid until the next config_document call
            unsafe { std::slice::from_raw_parts(ptr, len) }
        };
        let text = std::str::from_utf8(bytes).map_err(|error| LinkError::Json { reason: error.to_string() })?;
        hcl::from_str(text).map_err(|error| LinkError::Json { reason: error.to_string() })
    }

    /// Changes one of the mod's own saved values at `path`.
    pub fn set_setting(&self, path: &str, value: &hcl::Value) -> Result<(), LinkError> {
        let Some(set_setting) = self.table.api().set_setting else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let expression =
            hcl::format::to_string(value).map_err(|error| LinkError::Json { reason: error.to_string() })?;
        // SAFETY: path and the expression text live for the call; the handle is this link's until drop
        let status =
            unsafe { set_setting(self.handle, path.as_ptr(), path.len(), expression.as_ptr(), expression.len()) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Whether `vk` is down. The manager answers only while this mod holds the `Input` lease.
    pub fn key_down(&self, vk: u32) -> bool {
        match self.table.api().key_down {
            // SAFETY: key_down takes plain values; the handle is this link's until drop
            Some(key_down) => unsafe { key_down(self.handle, vk) },
            None => false,
        }
    }

    /// Sets the keys this mod wants `ModEvent` key reports for, replacing the earlier set; an empty slice clears
    /// it. The mod must hold the `Input` lease: `NotPermitted` otherwise. More than 32 keys or a key above 255 is
    /// `InvalidArgument`. A manager older than ABI 2.1 is `Unsupported`.
    pub fn watch_keys(&self, vks: &[u32]) -> Result<(), LinkError> {
        let Some(watch_keys) = self.table.api().watch_keys else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: the key slice lives for the call and the manager copies what it keeps; the handle is this
        // link's until drop
        let status = unsafe { watch_keys(self.handle, vks.as_ptr(), vks.len()) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Holds the game key `vk` down (`down`) or lets it go. The mod must hold the `Input` lease: `NotPermitted`
    /// otherwise, and while the panel is open or another mod holds `Input`. A key above 255 is `InvalidArgument`.
    /// When the manager lets go of a held key itself, the mod gets `ModEvent::FakeKeysReleased`. A manager older
    /// than ABI 2.1 is `Unsupported`.
    pub fn send_key(&self, vk: u32, down: bool) -> Result<(), LinkError> {
        let Some(send_key) = self.table.api().send_key else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: send_key takes plain values; the handle is this link's until drop
        let status = unsafe { send_key(self.handle, vk, u8::from(down)) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Holds a game mouse button down, lets it go, turns the wheel by `steps` notches (positive away from the
    /// player), or turns the player's look on one axis (`PointerOp::Turn`, in millidegrees). The rules of `send_key`
    /// apply: the mod must hold the `Input` lease, `NotPermitted` otherwise, and while the panel is open or another
    /// mod holds `Input`. A wheel turn past 32 notches either way, or a look turn past 360000 millidegrees either
    /// way, is `InvalidArgument`. The manager adds the look turns of one axis into one pending value, held to
    /// 360000 either way, and applies it once per client update (a frame). A manager queue full of button actions
    /// is `QueueFull`, and nothing changes. When the manager lets go of a held button itself, the mod gets
    /// `ModEvent::FakeKeysReleased`. A manager older than ABI 2.2 is `Unsupported`. A manager of ABI 2.2 answers
    /// `InvalidArgument` to the look turns; they need ABI 2.3.
    pub fn send_pointer(&self, op: PointerOp) -> Result<(), LinkError> {
        let Some(send_pointer) = self.table.api().send_pointer else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let (code, value) = PointerOpCodec::encode(op);
        // SAFETY: send_pointer takes plain values; the handle is this link's until drop
        let status = unsafe { send_pointer(self.handle, code, value) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Turns the cursor mode on or off.
    pub fn set_cursor_mode(&self, on: bool) -> Result<(), LinkError> {
        let Some(set_cursor_mode) = self.table.api().set_cursor_mode else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: set_cursor_mode takes plain values; the handle is this link's until drop
        unsafe { set_cursor_mode(self.handle, u8::from(on)) };
        Ok(())
    }

    /// Asks for a game-only screenshot of the next Present, taken before any mod layer, written as a PNG to `path`,
    /// and returns its sequence, which the `ScreenshotTaken` and `ScreenshotWritten` events carry. `path` is an
    /// absolute `.png` file path whose parent folder exists; the mod must hold an active `Hud` or `Overlay` lease.
    /// A path that is not UTF-8 is `InvalidArgument`; a manager older than ABI 1.2 is `Unsupported`.
    pub fn request_screenshot(&self, path: &Path) -> Result<u32, LinkError> {
        let Some(request_screenshot) = self.table.api().request_screenshot else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let Some(path) = path.to_str() else {
            return Err(LinkError::from(AmmStatus::InvalidArgument));
        };
        let mut sequence = 0;
        // SAFETY: the path text and the out sequence live for the call; the handle is this link's until drop
        let status = unsafe { request_screenshot(self.handle, path.as_ptr(), path.len(), &mut sequence) };
        match status {
            AmmStatus::Ok => Ok(sequence),
            other => Err(LinkError::from(other)),
        }
    }

    /// `request_screenshot` with a choice of capture point. `GameOnly` is taken before any mod layer;
    /// `WithModLayers` after the last mod layer and before the manager's panel and toasts, so the picture shows the
    /// layers of all mods, as the player sees them. The rules and the events are those of `request_screenshot`. A
    /// manager older than ABI 1.3 is `Unsupported` for both choices.
    pub fn request_screenshot_with(&self, path: &Path, layers: ScreenshotLayers) -> Result<u32, LinkError> {
        let Some(request_screenshot_layers) = self.table.api().request_screenshot_layers else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let Some(path) = path.to_str() else {
            return Err(LinkError::from(AmmStatus::InvalidArgument));
        };
        let layers = AmmScreenshotLayers::from(layers) as u8;
        let mut sequence = 0;
        // SAFETY: the path text and the out sequence live for the call; the handle is this link's until drop
        let status =
            unsafe { request_screenshot_layers(self.handle, path.as_ptr(), path.len(), layers, &mut sequence) };
        match status {
            AmmStatus::Ok => Ok(sequence),
            other => Err(LinkError::from(other)),
        }
    }

    /// The manager's capture stream, for game-only frames one per Present; dropping it ends the stream. See
    /// `CaptureStream`.
    pub fn capture_stream(&self) -> CaptureStream {
        CaptureStream::new(self.table, self.handle)
    }

    /// True when the manager's table has `request_screenshot`, ABI 1.2 or later. A mod can check it before it
    /// offers a screenshot key.
    pub fn supports_screenshots(&self) -> bool {
        self.table.api().request_screenshot.is_some()
    }

    /// True when a mod other than this one holds `capability` now. False on a manager older than ABI 1.2. Call it
    /// on the game thread: from a frame callback (render thread) the host is busy and the answer is false.
    pub fn held_by_other(&self, capability: Capability) -> bool {
        let Some(held_by_other) = self.table.api().held_by_other else {
            return false;
        };
        // SAFETY: held_by_other takes plain values; the handle is this link's until drop
        let held = unsafe { held_by_other(self.handle, AmmCapability::from(capability) as u8) };
        held == 1
    }

    /// True while the manager panel is open. False on a manager older than ABI 2.2. Call it on the game thread: from
    /// a frame callback (render thread) the host is busy and the answer is false.
    pub fn panel_open(&self) -> bool {
        let Some(panel_open) = self.table.api().panel_open else {
            return false;
        };
        // SAFETY: panel_open takes a plain value; the handle is this link's until drop
        let open = unsafe { panel_open(self.handle) };
        open == 1
    }

    /// Places a button for this mod in the corner row, left of the Mods button, on `screens`. Returns the entry
    /// id, which `ModEvent::MenuEntryClicked` carries, or `None` on a manager older than ABI 1.4.
    pub fn register_menu_entry(
        &self,
        label: &str,
        mark: &str,
        screens: MenuScreens,
    ) -> Result<Option<MenuEntryId>, LinkError> {
        let Some(register_menu_entry) = self.table.api().register_menu_entry else {
            return Ok(None);
        };
        let mut id = 0u16;
        // SAFETY: the label, the mark and the out id live for the call; the handle is this link's until drop
        let status = unsafe {
            register_menu_entry(
                self.handle,
                label.as_ptr(),
                label.len(),
                mark.as_ptr(),
                mark.len(),
                screens.bits(),
                &mut id,
            )
        };
        match status {
            AmmStatus::Ok => Ok(Some(MenuEntryId(id))),
            other => Err(LinkError::from(other)),
        }
    }

    /// The texture budget the manager gave this mod. A manager older than ABI 1.2 is `Unsupported`.
    pub fn texture_budget(&self) -> Result<TextureGrant, LinkError> {
        let Some(texture_budget) = self.table.api().texture_budget else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let (mut bytes, mut count) = (0, 0);
        // SAFETY: both out pointers live for the call; the handle is this link's until drop
        let status = unsafe { texture_budget(self.handle, &mut bytes, &mut count) };
        match status {
            AmmStatus::Ok => Ok(TextureGrant { bytes, count }),
            other => Err(LinkError::from(other)),
        }
    }

    /// The live game picture this mod may draw, with the back buffer size of the last Present (0 x 0 before the
    /// first). Draw it as `MeshFlattener::game_picture()`; the mod uploads nothing. `NotPermitted` while the mod
    /// does not hold `Overlay`. A manager older than ABI 1.7 is `Unsupported`. Game thread only: a call from inside a
    /// frame callback is `Busy`.
    pub fn game_picture(&self) -> Result<GamePicture, LinkError> {
        let Some(game_picture) = self.table.api().game_picture else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let (mut width, mut height) = (0, 0);
        // SAFETY: both out pointers live for the call; the handle is this link's until drop
        let status = unsafe { game_picture(self.handle, &mut width, &mut height) };
        match status {
            AmmStatus::Ok => Ok(GamePicture::new(width, height)),
            other => Err(LinkError::from(other)),
        }
    }

    /// Sets where this mod draws the game picture, `[min_x, min_y, max_x, max_y]` in physical pixels, or clears
    /// it with `None`. While the mod holds `Overlay`, a pointer inside it reaches the game at the matching place of
    /// the full window. A manager older than ABI 1.7 is `Unsupported`. Game thread only: a call from inside a frame
    /// callback is `Busy`.
    pub fn set_game_viewport(&self, rect: Option<[f32; 4]>) -> Result<(), LinkError> {
        let Some(set_game_viewport) = self.table.api().set_game_viewport else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let pointer = rect.as_ref().map_or(std::ptr::null(), |rect| rect as *const [f32; 4]);
        // SAFETY: the rect lives for the call, or the pointer is null; the handle is this link's until drop
        let status = unsafe { set_game_viewport(self.handle, pointer) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Makes the game draw at `size`, in physical pixels, whatever the size of its window, or ends the size this
    /// mod forces with `None`. The mod must hold `Capability::RenderSize`; the size also ends when the lease goes.
    /// The game resizes within a few Presents: wait until `render_size` reports `actual` equal to `wanted` before
    /// taking a picture at the size. A side of 0 or above `RenderSize::MAX_SIDE` is `InvalidArgument`. A manager
    /// older than ABI 1.8 is `Unsupported`. Game thread only.
    pub fn set_render_size(&self, size: Option<RenderSize>) -> Result<(), LinkError> {
        let Some(set_render_size) = self.table.api().set_render_size else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let size = size.map(AmmRenderSize::from);
        let pointer = size.as_ref().map_or(std::ptr::null(), |size| size as *const AmmRenderSize);
        // SAFETY: the size lives for the call, or the pointer is null; the handle is this link's until drop
        let status = unsafe { set_render_size(self.handle, pointer) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// The size this mod forces and the back buffer size and DXGI format of the last Present. `NotPermitted`
    /// while the mod does not hold `Capability::RenderSize`. A manager older than ABI 1.8 is `Unsupported`. Game
    /// thread only.
    pub fn render_size(&self) -> Result<RenderSizeState, LinkError> {
        let Some(render_size) = self.table.api().render_size else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let mut state = AmmRenderSizeState::blank();
        // SAFETY: the out state lives for the call; the handle is this link's until drop
        let status = unsafe { render_size(self.handle, &mut state) };
        match status {
            AmmStatus::Ok => Ok(RenderSizeState::from(state)),
            other => Err(LinkError::from(other)),
        }
    }

    /// The overlay's Present count: the number of the newest Present it started, 0 before the first. A capture
    /// stream request for a later Present is `CaptureStream::request_at`. `Unsupported` when the manager has no
    /// such slot. Game thread only.
    pub fn present_count(&self) -> Result<u64, LinkError> {
        let Some(present_count) = self.table.api().present_count else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let mut count = 0u64;
        // SAFETY: the out count lives for the call; the handle is this link's until drop
        match unsafe { present_count(self.handle, &mut count) } {
            AmmStatus::Ok => Ok(count),
            other => Err(LinkError::from(other)),
        }
    }

    /// Sets the mod's icon from PNG bytes: at most `IconDecoder::MAX_BYTES`, each side 1 to 256 px, first frame
    /// only. A refused icon keeps the one shown before. A manager older than ABI 1.6 is `Unsupported`.
    pub fn set_icon(&self, png: &[u8]) -> Result<(), LinkError> {
        let Some(set_icon) = self.table.api().set_icon else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: the PNG bytes live for the call and the manager copies what it keeps; the handle is this link's
        // until drop
        let status = unsafe { set_icon(self.handle, png.as_ptr(), png.len()) };
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// `set_icon` over the PNG file at `path`. The mod reads the file here; the manager never opens it. A file
    /// larger than `IconDecoder::MAX_BYTES`, an empty file and a file that cannot be read are `IconFile`.
    /// To clear the icon, call `set_icon` with no bytes.
    pub fn set_icon_file(&self, path: &Path) -> Result<(), LinkError> {
        let limit = IconDecoder::MAX_BYTES as u64;
        let file_error = |reason: String| LinkError::IconFile { reason };
        let mut file = std::fs::File::open(path).map_err(|error| file_error(error.to_string()))?;
        let size = file.metadata().map_err(|error| file_error(error.to_string()))?.len();
        if size > limit {
            return Err(file_error(format!("{size} bytes is over the {limit} byte limit")));
        }
        let mut png = Vec::new();
        std::io::Read::take(&mut file, limit + 1)
            .read_to_end(&mut png)
            .map_err(|error| file_error(error.to_string()))?;
        if png.len() as u64 > limit {
            return Err(file_error(format!("more than {limit} bytes")));
        }
        if png.is_empty() {
            return Err(file_error("the file is empty".to_owned()));
        }
        self.set_icon(&png)
    }

    // The id text of a holder, or `None` for an empty or non-UTF-8 id.
    fn id_text(bytes: &[u8]) -> Option<String> {
        if bytes.is_empty() {
            return None;
        }
        std::str::from_utf8(bytes).ok().map(str::to_owned)
    }

    fn install_logging(sink: &ForwardSink) {
        match Logger::install(Box::new(Dispatcher::new(vec![sink.clone()]))) {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        match TracingBridge::install_log_capture() {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        // The only failure is a global subscriber already set by this DLL, which counts as installed
        if tracing_subscriber::registry().with(TracingBridge::to_global()).try_init().is_err() {
            curia::debug!("tracing subscriber already set, kept as it is");
        }
    }
}

impl Drop for ManagerLink {
    fn drop(&mut self) {
        let Some(unregister) = self.table.api().unregister else {
            // The manager could still call the callbacks, so the frame state is left alive
            curia::error!("manager table has no unregister, the registration and its frame state, if any, are kept");
            self.sink.detach();
            return;
        };
        // SAFETY: the handle is this link's, and drop runs once
        let status = unsafe { unregister(self.handle) };
        if status != AmmStatus::Ok {
            // The registration may still be live, so the manager could still call the callbacks: the frame state
            // is leaked rather than freed under them
            curia::error!("unregister refused, the registration and its frame state, if any, are kept", { status: status as i32 });
            self.sink.detach();
            return;
        }
        self.sink.detach();
        if let Some(free) = self.free {
            // SAFETY: unregister returned Ok, so the manager no longer calls the callbacks, and user is freed here
            // once
            unsafe { free(self.user) };
        }
    }
}
