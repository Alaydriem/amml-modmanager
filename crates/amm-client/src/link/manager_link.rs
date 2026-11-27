use std::io::Read;
use std::path::Path;

use amm_abi::types::{AmmCapability, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus};
use amm_core::errors::LinkError;
use amm_core::services::icon::IconDecoder;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::{Capability, Descriptor};
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::mesh::GamePicture;
use amm_core::structs::render::{RenderSize, RenderSizeState};
use amm_core::traits::AbiSized;
use amml_api::amml_abi::{
    InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, ModsLoaded, ProviderGone,
    ProviderReady, ScreenshotTaken, ScreenshotWritten,
};
use amml_api::{
    ApiError, CaptureRequest, CaptureStream, Ctx, Event, EventCtx, HookOrder, InjectedInputReleasedExt, KeyCode,
    Listener, MoveIntent, ProviderGoneExt, ProviderReadyExt, ShotId, ShotLayers,
};
use serde::de::DeserializeOwned;

#[cfg(feature = "egui")]
use super::FrameHandler;
use super::{EventDrain, FrameSlot, InputLink, LinkCell, LinkShared, LinkState, LogBridge, ManagerTables, PointerOp};
#[cfg(feature = "egui")]
use crate::bridge::{EguiFrame, FrameTrampoline, LocaleSlot};

/// A mod's link to the manager through the loader's registry. `start` in `enable` adds the link's listeners; the
/// link connects itself when the manager's tables are published (`ProviderReady`), registers the mod's descriptor
/// with the mod's config folder, gives the manager the mod's layer callbacks, and keeps the descriptor's `Input`
/// claim as one loader input lease. At the manager's `ProviderGone` the link unregisters and is gone; every call
/// then gives `LinkError::Gone` and reaches no table. Drop removes the listeners, then unregisters.
///
/// Call it on the game thread. The link takes its lock only between calls, never during a loader or manager call, so a
/// listener may call the link from inside an event the loader raises during one of the link's calls.
pub struct ManagerLink {
    // Dropped first, so no listener runs while the link closes
    listeners: Vec<Listener>,
    cell: LinkCell,
    // The localizer the frame's layers read; a link without a frame has a slot no pass reads
    #[cfg(feature = "egui")]
    locale: LocaleSlot,
}

impl ManagerLink {
    // Room for the longest mod id the descriptor checker accepts, so one call is enough
    const HOLDER_ID_BYTES: usize = 64;
    // Room for a locale id such as `pt_BR` or `zh_Hant_TW`, so one call is enough
    const LOCALE_BYTES: usize = 16;

    /// Starts a link for the mod of `ctx` with `descriptor_hcl` as its descriptor and `handler`, when given, as its
    /// egui pass. Call it in `enable`: it adds the link's listeners and installs `LogBridge` over the mod's logger.
    /// The link is `Waiting` until the manager's tables are published. A descriptor that does not parse is
    /// `Json`; a listen of `ProviderReady` or `ProviderGone` the loader refuses is `Api`.
    #[cfg(feature = "egui")]
    pub fn start<H: FrameHandler + Send + 'static>(
        ctx: &Ctx,
        descriptor_hcl: &'static str,
        handler: Option<H>,
    ) -> Result<ManagerLink, LinkError> {
        let mut locale = LocaleSlot::new();
        let frame = handler.map(|handler| {
            let frame = EguiFrame::new(handler);
            locale = frame.locale().clone();
            let user = FrameTrampoline::<H>::into_user(frame);
            FrameSlot::new(FrameTrampoline::<H>::callbacks(user), FrameTrampoline::<H>::free)
        });
        let mut link = Self::begin(ctx, descriptor_hcl, frame)?;
        link.locale = locale;
        Ok(link)
    }

    /// `start` for a mod that draws nothing through the manager: no layer callbacks.
    pub fn start_without_frame(ctx: &Ctx, descriptor_hcl: &'static str) -> Result<ManagerLink, LinkError> {
        Self::begin(ctx, descriptor_hcl, None)
    }

    fn begin(ctx: &Ctx, descriptor_hcl: &'static str, frame: Option<FrameSlot>) -> Result<ManagerLink, LinkError> {
        let parsed = hcl::from_str::<Descriptor>(descriptor_hcl)
            .map_err(|error| LinkError::Json { reason: error.to_string() })
            .and_then(|descriptor| Self::same_id(ctx, descriptor));
        let descriptor = match parsed {
            Ok(descriptor) => descriptor,
            Err(error) => {
                if let Some(frame) = frame {
                    // SAFETY: the manager never got the callbacks, so none runs with this state
                    unsafe { frame.free() };
                }
                return Err(error);
            }
        };
        LogBridge::install(ctx.log());
        let input = InputLink::new(descriptor.input_claim().as_ref());
        let cell = LinkCell::new(LinkShared::new(*ctx, descriptor_hcl, input, frame));
        let mut link = Self {
            listeners: Vec::new(),
            cell,
            #[cfg(feature = "egui")]
            locale: LocaleSlot::new(),
        };
        link.listen_all(ctx)?;
        // A manager whose tables are there already (a link started after `ModsLoaded`) gets no new `ProviderReady`
        link.cell.try_connect();
        Ok(link)
    }

    // The manager ranks input leases by descriptor id and the loader by its own mod id, so the two must be the same.
    fn same_id(ctx: &Ctx, descriptor: Descriptor) -> Result<Descriptor, LinkError> {
        let id = descriptor.id();
        if id == ctx.module() {
            return Ok(descriptor);
        }
        ctx.log().error(
            "the descriptor names another mod than the loader's",
            &[("descriptor", &id), ("module", ctx.module())],
        );
        Err(LinkError::IdMismatch { descriptor: id, module: ctx.module().to_owned() })
    }

    // On an error the link is dropped, which closes it and frees a frame state the manager never got.
    fn listen_all(&mut self, ctx: &Ctx) -> Result<(), LinkError> {
        self.listen::<ProviderReady>(ctx, true, |shared, event| {
            if let Some(name) = event.name() {
                shared.provider_ready(name);
            }
        })?;
        self.listen::<ProviderGone>(ctx, true, |shared, event| {
            if let Some(name) = event.name() {
                shared.provider_gone(name);
            }
        })?;
        self.listen::<ModsLoaded>(ctx, false, |shared, _| shared.mods_loaded())?;
        self.listen::<InputLeaseSuspended>(ctx, false, |shared, event| {
            if let Some(event) = EventDrain::suspended(event, shared.lease_id()) {
                shared.push(event);
            }
        })?;
        self.listen::<InputLeaseResumed>(ctx, false, |shared, event| {
            if let Some(event) = EventDrain::resumed(event, shared.lease_id()) {
                shared.push(event);
            }
        })?;
        self.listen::<InputLeaseLost>(ctx, false, |shared, event| {
            if let Some(event) = EventDrain::lost(event, shared.lease_id()) {
                shared.input_lost();
                shared.push(event);
            }
        })?;
        self.listen::<InjectedInputReleased>(ctx, false, |shared, event| {
            if let Some(released) = EventDrain::released(event, shared.lease_id()) {
                shared.input_released(event.released_key(), event.released_button());
                shared.push(released);
            }
        })?;
        self.listen::<ScreenshotTaken>(ctx, false, |shared, event| {
            if let Some(event) = EventDrain::taken(event) {
                shared.push(event);
            }
        })?;
        self.listen::<ScreenshotWritten>(ctx, false, |shared, event| {
            if let Some(event) = EventDrain::written(event) {
                shared.push(event);
            }
        })?;
        Ok(())
    }

    // A listen of an event the loader does not serve is skipped when `needed` is false.
    fn listen<E: Event>(
        &mut self,
        ctx: &Ctx,
        needed: bool,
        mut f: impl FnMut(&LinkCell, &E) + Send + 'static,
    ) -> Result<(), LinkError> {
        let cell = self.cell.clone();
        let listened = ctx.events().listen::<E>(HookOrder::NORMAL, move |event: &mut E, _: &EventCtx| {
            f(&cell, event);
        });
        match listened {
            Ok(listener) => {
                self.listeners.push(listener);
                Ok(())
            }
            Err(ApiError::NotAvailable) if !needed => Ok(()),
            Err(error) => Err(LinkError::Api(error)),
        }
    }

    fn connection(&self) -> Result<(ManagerTables, AmmHandle), LinkError> {
        self.cell.connection()
    }

    /// Where the link is with the manager.
    pub fn state(&self) -> LinkState {
        self.cell.state()
    }

    /// The loader's id of the input lease the link holds, as the lease events carry it; `None` without one.
    pub fn input_lease_id(&self) -> Option<u64> {
        self.cell.lease_id()
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

    /// Asks for a lease on `capability`. `Input` takes the loader input lease of the descriptor's `Input` claim
    /// (`InputLink::acquire`); the other capabilities are the manager's leases.
    pub fn acquire(&self, capability: Capability) -> Result<AcquireOutcome, LinkError> {
        if capability == Capability::Input {
            return self.cell.acquire_input();
        }
        let (tables, handle) = self.connection()?;
        let acquire = tables.leases.acquire.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let capability = AmmCapability::try_from(capability).map_err(LinkError::from)? as u8;
        // SAFETY: acquire takes plain values; the handle is this link's until it unregisters
        let result = unsafe { acquire(handle, capability) };
        Ok(AcquireOutcome::from(result))
    }

    /// Gives a lease back. `Input` lets go of every key and button the link holds and ends the input lease.
    pub fn release(&self, capability: Capability) {
        if capability == Capability::Input {
            self.cell.release_input();
            return;
        }
        let Ok((tables, handle)) = self.connection() else {
            return;
        };
        let (Some(release), Ok(capability)) = (tables.leases.release, AmmCapability::try_from(capability)) else {
            return;
        };
        // SAFETY: release takes plain values; the handle is this link's until it unregisters
        unsafe { release(handle, capability as u8) };
    }

    /// The id of the mod that holds the manager lease `capability` now; `None` when no mod holds it, for `Input`
    /// (a loader lease), and while the link is not connected.
    pub fn holder(&self, capability: Capability) -> Option<String> {
        let (tables, _) = self.connection().ok()?;
        let holder = tables.leases.holder?;
        let capability = AmmCapability::try_from(capability).ok()? as u8;
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

    /// True when a mod other than this one holds the manager lease `capability` now. False for `Input` and while
    /// the link is not connected. Call it on the game thread: from a frame callback the host is busy and the answer
    /// is false.
    pub fn held_by_other(&self, capability: Capability) -> bool {
        let Ok((tables, handle)) = self.connection() else {
            return false;
        };
        let (Some(held_by_other), Ok(capability)) = (tables.leases.held_by_other, AmmCapability::try_from(capability))
        else {
            return false;
        };
        // SAFETY: held_by_other takes plain values; the handle is this link's until it unregisters
        unsafe { held_by_other(handle, capability as u8) == 1 }
    }

    /// The manager events and the loader events the link passes on (input lease, injected release, screenshots,
    /// `ManagerGone`), in arrival order. A cancelled loader event is not passed on.
    pub fn events(&mut self) -> impl Iterator<Item = ModEvent> {
        self.cell.take_events().into_iter()
    }

    /// The language code the person picked in the manager: empty for English, `en_XA` for the pseudo-locale, or a
    /// locale such as `ru`. After this call the mod gets `ModEvent::LocaleChanged` when the code changes. `None`
    /// while the link is not connected.
    pub fn locale(&self) -> Option<String> {
        let (tables, handle) = self.connection().ok()?;
        let locale = tables.core.locale?;
        let mut buffer = [0u8; Self::LOCALE_BYTES];
        // SAFETY: the buffer has room for its length in bytes for the call; the handle is this link's
        let len = unsafe { locale(handle, buffer.as_mut_ptr(), buffer.len()) } as usize;
        if len <= buffer.len() {
            return std::str::from_utf8(&buffer[..len]).ok().map(str::to_owned);
        }
        let mut long = vec![0u8; len];
        // SAFETY: the buffer has room for its length in bytes for the call; the handle is this link's
        let again = unsafe { locale(handle, long.as_mut_ptr(), long.len()) } as usize;
        if again > long.len() {
            return None;
        }
        std::str::from_utf8(&long[..again]).ok().map(str::to_owned)
    }

    /// True while the manager panel is open. False while the link is not connected.
    pub fn panel_open(&self) -> bool {
        let Ok((tables, _)) = self.connection() else {
            return false;
        };
        let Some(panel_open) = tables.core.panel_open else {
            return false;
        };
        // SAFETY: panel_open takes no arguments
        unsafe { panel_open() == 1 }
    }

    /// The mod's checked `config.hcl` parsed into its own type.
    pub fn config<T: DeserializeOwned>(&self) -> Result<T, LinkError> {
        let (tables, handle) = self.connection()?;
        let config_document = tables.settings.config_document.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let mut ptr = std::ptr::null();
        let mut len = 0;
        // SAFETY: both out pointers live for the call; the handle is this link's
        let status = unsafe { config_document(handle, &mut ptr, &mut len) };
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
        let (tables, handle) = self.connection()?;
        let set_setting = tables.settings.set_setting.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let expression =
            hcl::format::to_string(value).map_err(|error| LinkError::Json { reason: error.to_string() })?;
        // SAFETY: path and the expression text live for the call; the handle is this link's
        let status = unsafe { set_setting(handle, path.as_ptr(), path.len(), expression.as_ptr(), expression.len()) };
        Self::check(status)
    }

    /// Holds the game key `key` down (`down`) or lets it go, through the input lease. A second down of a held key
    /// and an up of a key not held change nothing. Without the lease (`acquire(Capability::Input)`) or while it is
    /// suspended `Api(Refused)`. When the loader lets go of the keys itself, the mod gets
    /// `ModEvent::FakeKeysReleased`.
    pub fn send_key(&self, key: KeyCode, down: bool) -> Result<(), LinkError> {
        self.cell.send_key(key, down)
    }

    /// Holds a mouse button down or lets it go, turns the wheel, or turns the view, through the input lease, under
    /// the rules of `send_key`.
    pub fn send_pointer(&self, op: PointerOp) -> Result<(), LinkError> {
        self.cell.send_pointer(op)
    }

    /// Holds a movement of the local player until the next call. The all-zero intent clears it and is always `Ok`;
    /// any other intent needs the input lease, under the rules of `send_key`.
    pub fn send_move(&self, intent: MoveIntent) -> Result<(), LinkError> {
        self.cell.send_move(intent)
    }

    /// Turns the cursor mode on or off.
    pub fn set_cursor_mode(&self, on: bool) -> Result<(), LinkError> {
        let (tables, handle) = self.connection()?;
        let set_cursor_mode = tables.layers.set_cursor_mode.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        // SAFETY: set_cursor_mode takes plain values; the handle is this link's
        unsafe { set_cursor_mode(handle, u8::from(on)) };
        Ok(())
    }

    /// Asks the loader for a PNG of the next frame with a surface at `path` (absolute): `GameOnly` before any mod
    /// layer, `WithModLayers` with the mod layers and without the manager's panel. The returned id is in the
    /// `ScreenshotTaken` and `ScreenshotWritten` events. It needs no manager; the loader's refusals are `Api`.
    pub fn request_screenshot_with(&self, path: &Path, layers: ScreenshotLayers) -> Result<ShotId, LinkError> {
        let layers = match layers {
            ScreenshotLayers::GameOnly => ShotLayers::GameOnly,
            ScreenshotLayers::WithModLayers => ShotLayers::WithOverlays,
        };
        let overlay = self.cell.ctx().overlay();
        overlay.screenshot(path, layers).map_err(LinkError::Api)
    }

    /// Opens a loader capture stream; dropping it closes the stream. It needs no manager; the loader's refusals are
    /// `Api`.
    pub fn capture_stream(&self, request: CaptureRequest) -> Result<CaptureStream, LinkError> {
        let overlay = self.cell.ctx().overlay();
        overlay.open_capture_stream(request).map_err(LinkError::Api)
    }

    /// Places a button for this mod in the corner row, left of the Mods button, on `screens`. Returns the entry
    /// id, which `ModEvent::MenuEntryClicked` carries, or `None` when the manager's table has no such slot.
    pub fn register_menu_entry(
        &self,
        label: &str,
        mark: &str,
        screens: MenuScreens,
    ) -> Result<Option<MenuEntryId>, LinkError> {
        let (tables, handle) = self.connection()?;
        let Some(register_menu_entry) = tables.core.register_menu_entry else {
            return Ok(None);
        };
        let mut id = 0u16;
        // SAFETY: the label, the mark and the out id live for the call; the handle is this link's
        let status = unsafe {
            register_menu_entry(handle, label.as_ptr(), label.len(), mark.as_ptr(), mark.len(), screens.bits(), &mut id)
        };
        Self::check(status).map(|()| Some(MenuEntryId(id)))
    }

    /// The texture budget the manager gave this mod.
    pub fn texture_budget(&self) -> Result<TextureGrant, LinkError> {
        let (tables, handle) = self.connection()?;
        let texture_budget = tables.layers.texture_budget.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let (mut bytes, mut count) = (0, 0);
        // SAFETY: both out pointers live for the call; the handle is this link's
        let status = unsafe { texture_budget(handle, &mut bytes, &mut count) };
        Self::check(status).map(|()| TextureGrant { bytes, count })
    }

    /// The live game picture this mod may draw, with the back buffer size of the last Present (0 x 0 before the
    /// first). `NotPermitted` while the mod does not hold `Overlay`. Game thread only.
    pub fn game_picture(&self) -> Result<GamePicture, LinkError> {
        let (tables, handle) = self.connection()?;
        let game_picture = tables.layers.game_picture.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let (mut width, mut height) = (0, 0);
        // SAFETY: both out pointers live for the call; the handle is this link's
        let status = unsafe { game_picture(handle, &mut width, &mut height) };
        Self::check(status).map(|()| GamePicture::new(width, height))
    }

    /// Sets where this mod draws the game picture, `[min_x, min_y, max_x, max_y]` in back-buffer pixels, or clears
    /// it with `None`. Game thread only.
    pub fn set_game_viewport(&self, rect: Option<[f32; 4]>) -> Result<(), LinkError> {
        let (tables, handle) = self.connection()?;
        let set_game_viewport = tables.layers.set_game_viewport.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let pointer = rect.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
        // SAFETY: the rect lives for the call, or the pointer is null; the handle is this link's
        let status = unsafe { set_game_viewport(handle, pointer) };
        Self::check(status)
    }

    /// Makes the game draw at `size`, in physical pixels, or ends the size this mod forces with `None`. The mod
    /// must hold `Capability::RenderSize`. Game thread only.
    pub fn set_render_size(&self, size: Option<RenderSize>) -> Result<(), LinkError> {
        let (tables, handle) = self.connection()?;
        let set_render_size = tables.layers.set_render_size.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let size = size.map(AmmRenderSize::from);
        let pointer = size.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
        // SAFETY: the size lives for the call, or the pointer is null; the handle is this link's
        let status = unsafe { set_render_size(handle, pointer) };
        Self::check(status)
    }

    /// The size this mod forces and the back buffer size and format of the last Present. `NotPermitted` while the
    /// mod does not hold `Capability::RenderSize`. Game thread only.
    pub fn render_size(&self) -> Result<RenderSizeState, LinkError> {
        let (tables, handle) = self.connection()?;
        let render_size = tables.layers.render_size.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        let mut state = AmmRenderSizeState::blank();
        // SAFETY: the out state lives for the call; the handle is this link's
        let status = unsafe { render_size(handle, &mut state) };
        Self::check(status).map(|()| RenderSizeState::from(state))
    }

    /// Sets the mod's icon from PNG bytes: at most `IconDecoder::MAX_BYTES`, each side 1 to 256 px, first frame
    /// only. No bytes clear the icon.
    pub fn set_icon(&self, png: &[u8]) -> Result<(), LinkError> {
        let (tables, handle) = self.connection()?;
        let set_icon = tables.core.set_icon.ok_or(LinkError::from(AmmStatus::Unsupported))?;
        // SAFETY: the PNG bytes live for the call and the manager copies what it keeps; the handle is this link's
        let status = unsafe { set_icon(handle, png.as_ptr(), png.len()) };
        Self::check(status)
    }

    /// `set_icon` over the PNG file at `path`. The mod reads the file here; the manager never opens it. A file
    /// larger than `IconDecoder::MAX_BYTES`, an empty file and a file that cannot be read are `IconFile`.
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

    fn check(status: AmmStatus) -> Result<(), LinkError> {
        match status {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    // The id text of a holder, or `None` for an empty or non-UTF-8 id.
    fn id_text(bytes: &[u8]) -> Option<String> {
        if bytes.is_empty() {
            return None;
        }
        std::str::from_utf8(bytes).ok().map(str::to_owned)
    }
}

impl Drop for ManagerLink {
    fn drop(&mut self) {
        // No lock is held here, so a listener in progress can finish
        self.listeners.clear();
        self.cell.close();
    }
}
