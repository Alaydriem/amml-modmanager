use std::fs;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use amm_abi::types::{AmmCallbacks, AmmCaptureStreamFrame, AmmEvent, AmmFrameOutput, AmmHandle, AmmStatus};
use amm_core::config::ManagerConfig;
use amm_core::errors::ManagerError;
use amm_core::services::capture::CaptureStreamBook;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::services::manager::ManagerService;
use amm_core::services::settings::{ChordText, ManagerConfigFile};
use amm_core::services::{PanicGuard, PathRules};
use amm_core::structs::capture::{CaptureStreamRequest, ScreenshotLayers};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::frame::{FrameInputData, LayerKind, UiEvent, UiEventKind};
use amm_core::structs::icon::ModIcon;
use amm_core::structs::input::GameViewport;
use amm_core::structs::input::{FakeKeyOp, InputEvent, Modifiers, PointerOp, RouteDecision};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::structs::menu::MenuScreens;
use amm_core::structs::mesh::{GamePicture, MeshBuffers};
use amm_core::structs::registry::{ModIndex, ModMask};
use amm_core::structs::render::{RenderSize, RenderSizeState};
use amm_core::structs::settings::HclDocument;
use amm_core::structs::snapshot::LayerEntry;
use amm_store::logging::ModLogRouter;
use amm_store::services::{ConfigFile, IoQueue, IoThread};
use amm_store::structs::ReadOutcome;
use amm_ui::model::{PanelAction, PanelModel};
use amm_ui::runtime::PanelRuntime;
use amml_ui::font::FontSource;

use super::{
    CaptureRelay, ClientSlot, FrameCompositor, HolderBook, HostShared, LogBridge, PresentSequence, UiEventBuffer,
};
use crate::types::{AmmHostFrame, AmmHostFrameInput, AmmHostRoute};

/// Everything the manager DLL runs: the manager service, the config writer thread, the panel, the compositor and
/// one client slot per mod slot. Every method runs under the host lock. The state other threads read without the
/// lock is published to `HostShared` after each call. Bounds: the slots, the panel event buffer and the poll
/// buffer are fixed at creation from the manager config.
pub struct HostRuntime {
    manager: ManagerService,
    checker: DescriptorChecker,
    io: Option<IoThread>,
    panel: PanelRuntime,
    compositor: Option<FrameCompositor>,
    captures: CaptureRelay,
    stream: CaptureStreamBook,
    slots: Vec<ClientSlot>,
    // the config folder each live mod registered with, one entry per slot
    mod_dirs: Vec<Option<PathBuf>>,
    manager_dir: PathBuf,
    shared: Arc<HostShared>,
    holders: Arc<HolderBook>,
    logs: &'static LogBridge,
    panel_events: UiEventBuffer,
    panel_actions: Vec<PanelAction>,
    poll_buffer: Vec<ModEvent>,
    // fake key presses taken from the manager that the host has not collected yet; the manager bounds the queue
    fake_ops: Vec<FakeKeyOp>,
    // fake mouse actions taken from the manager that the host has not collected yet; the manager bounds the queue
    fake_pointer_ops: Vec<PointerOp>,
    manager_version: String,
    now_ms: u64,
    // the size of the last size message; a message with the same size again is a resend
    last_size_message: Option<RenderSize>,
    pointer: (f32, f32),
    // the back buffer size of the last Present, 0 x 0 before the first
    back_buffer: GamePicture,
    modifiers: u8,
    last_telemetry_ms: u64,
    // toggle key presses logged so far
    toggle_seen: u8,
}

impl HostRuntime {
    const TOGGLE_SEEN_LIMIT: u8 = 20;
    const CONFIG_FILE: &'static str = "config.hcl";
    const IO_FLUSH: Duration = Duration::from_secs(2);
    const POLL_CHUNK: usize = 64;
    const MS_PER_SEC: u64 = 1000;

    /// Reads the manager config from `config_dir`, the manager's own config folder, starts logging and the config
    /// writer, and builds the panel. It opens the manager's log slot, which shutdown closes, and starts threads, so
    /// it must not run under the OS loader lock.
    pub fn new(config_dir: PathBuf, game_dir: PathBuf, manager_version: String) -> Result<HostRuntime, ManagerError> {
        let (config, config_file, config_fault) = Self::read_config(&config_dir);
        let manager_level = config.log_level_for(LogBridge::MANAGER_ID);
        let logs = LogBridge::process(&game_dir, config.log_sessions_kept);
        logs.open(ModLogRouter::MANAGER_SLOT, LogBridge::MANAGER_NAMESPACE, LogBridge::MANAGER_NAME, manager_level);
        if let Some(fault) = config_fault {
            curia::warn!("manager config not read, defaults used", { error: fault });
        }
        let budgets = config.budgets.clone();
        let io = IoThread::new(IoQueue::new(budgets.io_queue)).map_err(|error| {
            curia::error!("config writer did not start", { error: error.to_string() });
            logs.close(ModLogRouter::MANAGER_SLOT);
            ManagerError::from(error)
        })?;
        let slot_count = budgets.max_mods.min(ModMask::CAPACITY);
        let manager = ManagerService::new(config, config_file);
        let holders = Arc::new(HolderBook::empty());
        let shared = Arc::new(HostShared::new(manager.snapshot_source(), Arc::clone(&holders)));
        curia::info!("mod manager started", { version: manager_version.as_str(), slots: slot_count });
        Ok(Self {
            checker: DescriptorChecker::new(budgets.max_keybinds, budgets.max_fields),
            io: Some(io),
            panel: PanelRuntime::new(FontSource::Embedded, manager.locale()),
            compositor: Some(FrameCompositor::new(&budgets)),
            captures: CaptureRelay::new(),
            stream: CaptureStreamBook::new(),
            slots: (0..slot_count).map(|_| ClientSlot::new(budgets.event_queue)).collect(),
            mod_dirs: vec![None; slot_count],
            manager_dir: config_dir,
            shared,
            holders,
            logs,
            panel_events: UiEventBuffer::new(budgets.event_queue),
            panel_actions: Vec::new(),
            poll_buffer: vec![ModEvent::Enabled; Self::POLL_CHUNK],
            fake_ops: Vec::new(),
            fake_pointer_ops: Vec::new(),
            manager_version,
            now_ms: 0,
            last_size_message: None,
            pointer: (0.0, 0.0),
            back_buffer: GamePicture::new(0, 0),
            modifiers: 0,
            last_telemetry_ms: 0,
            toggle_seen: 0,
            manager,
        })
    }

    pub fn shared(&self) -> &Arc<HostShared> {
        &self.shared
    }

    pub fn manager(&self) -> &ManagerService {
        &self.manager
    }

    pub fn manager_mut(&mut self) -> &mut ManagerService {
        &mut self.manager
    }

    pub fn pointer(&self) -> (f32, f32) {
        self.pointer
    }

    /// Registers a mod from its descriptor bytes. Its config file, the descriptor's `config_path` in the mod's own
    /// `config_dir`, is read, checked, and written back at once when the check changed it. `config_dir` must be
    /// an absolute local path with no `..` part, not a network, device or verbatim path, not the manager's own
    /// config folder or a folder inside it, and not the folder of another live mod, else `InvalidArgument`.
    pub fn register(
        &mut self,
        descriptor: &[u8],
        config_dir: &Path,
        callbacks: AmmCallbacks,
    ) -> Result<AmmHandle, AmmStatus> {
        let mod_dir = self.checked_config_dir(config_dir)?;
        let text = std::str::from_utf8(descriptor).map_err(|_| AmmStatus::InvalidDescriptor)?;
        let parsed = self.checker.parse(text).map_err(|error| {
            curia::warn!("mod descriptor refused", { error: error.to_string() });
            AmmStatus::from(&error)
        })?;
        let registered = match Self::read_document(&mod_dir.join(&parsed.config_path)) {
            ReadOutcome::Parsed(document) => self.manager.register(text, Some(document), &mod_dir),
            ReadOutcome::Missing | ReadOutcome::Invalid => self.manager.register(text, None, &mod_dir),
            ReadOutcome::Unreadable => self.manager.register_unwritten(text, &mod_dir),
        };
        let (index, checked) = registered.map_err(|error| {
            curia::warn!("mod registration refused", { mod_id: parsed.id().as_str(), error: error.to_string() });
            AmmStatus::from(&error)
        })?;
        if checked.changed() {
            self.flush_writes(self.now_ms);
        }
        let Some(slot) = self.slots.get_mut(usize::from(index.0)) else {
            self.manager.unregister(index);
            return Err(AmmStatus::TooManyMods);
        };
        let generation = slot.occupy(callbacks);
        if let (Some(compositor), Some(grant)) = (self.compositor.as_mut(), self.manager.texture_grant(index)) {
            compositor.grant_textures(index.0, grant);
        }
        if let Some(dir) = self.mod_dirs.get_mut(usize::from(index.0)) {
            *dir = Some(mod_dir);
        }
        let handle = AmmHandle { slot: u16::from(index.0), generation };
        self.shared.set_generation(handle.slot, Some(generation));
        let id = parsed.id();
        self.logs.open(index.0, &parsed.namespace, &parsed.name, self.manager.config().log_level_for(&id));
        curia::info!("mod registered", { mod_id: id.as_str(), slot: index.0 });
        Ok(handle)
    }

    /// Removes a mod: its leases, cursor request, layer, log file and textures.
    pub fn unregister(&mut self, handle: AmmHandle) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.shared.set_generation(handle.slot, None);
        self.manager.unregister(index);
        self.stream.unregister(index);
        if let Some(slot) = self.slots.get_mut(usize::from(index.0)) {
            slot.vacate();
        }
        if let Some(dir) = self.mod_dirs.get_mut(usize::from(index.0)) {
            *dir = None;
        }
        self.logs.close(index.0);
        if let Some(compositor) = self.compositor.as_mut() {
            compositor.free_owner(index.0);
            self.manager.telemetry().record_retained_bytes(index, compositor.retained_bytes(index.0));
        }
        self.shared.mark_freed(handle.slot);
        Ok(())
    }

    /// The mod's checked config text, kept in its slot until the next call.
    pub fn config_document(&mut self, handle: AmmHandle) -> Result<(*const u8, usize), AmmStatus> {
        let index = self.index(handle)?;
        let text = self.manager.document(index).map(HclDocument::text).unwrap_or_default();
        let slot = self.slots.get_mut(usize::from(index.0)).ok_or(AmmStatus::UnknownHandle)?;
        let kept = slot.set_document(text);
        Ok((kept.as_ptr(), kept.len()))
    }

    pub fn acquire(&mut self, handle: AmmHandle, cap: Capability) -> Result<AcquireOutcome, AmmStatus> {
        let index = self.index(handle)?;
        self.manager.acquire(index, cap, self.now_ms).map_err(|error| AmmStatus::from(&error))
    }

    /// Books a screenshot of the next Present at the capture point `layers`, written to `path`, and returns its
    /// sequence. A mod without an active `Hud` or `Overlay` lease is `NotPermitted` before any path check. On top
    /// of the manager's path rules the parent folder of `path` must exist, else `InvalidArgument`.
    pub fn request_screenshot(
        &mut self,
        handle: AmmHandle,
        path: &Path,
        layers: ScreenshotLayers,
    ) -> Result<u32, AmmStatus> {
        let index = self.index(handle)?;
        if !self.manager.may_capture(index) {
            return Err(AmmStatus::NotPermitted);
        }
        if PathRules::screenshot_fault(path).is_none() && !path.parent().is_some_and(Path::is_dir) {
            curia::warn!("screenshot path refused", {
                path: path.display().to_string(),
                reason: "its parent folder does not exist",
            });
            return Err(AmmStatus::InvalidArgument);
        }
        self.manager.request_capture(index, path, layers).map_err(|error| AmmStatus::from(&error))
    }

    pub fn held_by_other(&self, handle: AmmHandle, cap: Capability) -> Result<bool, AmmStatus> {
        let index = self.index(handle)?;
        Ok(self.manager.held_by_other(index, cap))
    }

    /// The game picture `handle` may draw: the back buffer size of the last Present. `NotPermitted` while the mod
    /// does not hold `Overlay`.
    pub fn game_picture(&self, handle: AmmHandle) -> Result<GamePicture, AmmStatus> {
        let index = self.index(handle)?;
        if !self.manager.arbiter().holds(index, Capability::Overlay) {
            return Err(AmmStatus::NotPermitted);
        }
        Ok(self.back_buffer)
    }

    /// Sets or clears where `handle` draws the game picture.
    pub fn set_game_viewport(&mut self, handle: AmmHandle, viewport: Option<GameViewport>) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.set_game_viewport(index, viewport);
        Ok(())
    }

    /// Where the game's mouse feed must put the real cursor at back-buffer `point`, for a window with the `client`
    /// size, or `None` to leave the game's own point. See `RenderSizeBook::game_point`.
    pub fn game_point(&self, point: (f32, f32), client: RenderSize) -> Option<(f32, f32)> {
        let back_buffer = RenderSize::new(self.back_buffer.width, self.back_buffer.height);
        let size = (back_buffer.width as f32, back_buffer.height as f32);
        let mapped = self.manager.game_viewport_at(point).and_then(|viewport| viewport.map(point, size));
        self.manager.render_size().game_point(point, mapped, back_buffer, client)
    }

    /// The manager stops: the size message that gives the game its `client` size back, if one is due.
    pub fn render_size_shutdown(&mut self, client: RenderSize) -> Option<RenderSize> {
        let message = self.manager.render_size_shutdown(client);
        if let Some(size) = message {
            curia::info!("render size given back at shutdown", { width: size.width, height: size.height });
        }
        message
    }

    /// The manager starts again on a host that stayed alive: the render size book forces sizes again.
    pub fn render_size_open(&mut self) {
        self.manager.render_size_open();
    }

    /// Forces the render size of `handle`, or ends it with `None`. See `ManagerService::set_render_size`.
    pub fn set_render_size(&mut self, handle: AmmHandle, size: Option<RenderSize>) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.set_render_size(index, size).map_err(|error| AmmStatus::from(&error))
    }

    /// The forced size of `handle` and the back buffer of the last Present.
    pub fn render_size_state(&self, handle: AmmHandle) -> Result<RenderSizeState, AmmStatus> {
        let index = self.index(handle)?;
        let (actual, format) = self.shared.back_buffer();
        self.manager.render_size_state(index, actual, format).map_err(|error| AmmStatus::from(&error))
    }

    pub fn texture_budget(&self, handle: AmmHandle) -> Result<TextureGrant, AmmStatus> {
        let index = self.index(handle)?;
        self.manager.texture_grant(index).ok_or(AmmStatus::UnknownHandle)
    }

    /// The buffer the overlay copies the pixels of capture `sequence` into, and its row pitch. See
    /// `CaptureRelay::buffer`.
    pub fn capture_buffer(&mut self, sequence: u32, width: u32, height: u32) -> Result<(*mut u8, u32), AmmStatus> {
        self.captures.buffer(sequence, width, height).ok_or(AmmStatus::InvalidArgument)
    }

    /// The overlay copied capture `sequence` (`ok`) in the DXGI format `format`, or failed to.
    pub fn capture_done(&mut self, sequence: u32, ok: bool, format: u32) -> Result<(), AmmStatus> {
        self.captures.report(sequence, ok, format).then_some(()).ok_or(AmmStatus::InvalidArgument)
    }

    /// Books a capture stream frame of the next Present for `handle`. See `CaptureStreamBook::request`.
    pub fn request_stream_frame(&mut self, handle: AmmHandle, request: CaptureStreamRequest) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.stream.request(self.manager.arbiter(), index, request).map_err(|error| AmmStatus::from(&error))
    }

    /// The overlay's Present count, for the registered mod `handle`. See `PresentSequence::current`.
    pub fn present_count(&self, handle: AmmHandle) -> Result<u64, AmmStatus> {
        self.index(handle)?;
        Ok(PresentSequence::current())
    }

    /// The capture stream frames `handle` may ask for now.
    pub fn stream_free_slots(&self, handle: AmmHandle) -> Result<u32, AmmStatus> {
        let index = self.index(handle)?;
        Ok(u32::try_from(self.stream.free_slots(index)).unwrap_or(0))
    }

    /// The oldest capture stream frame of `handle` once it is done, its pixels lent until the next take that
    /// returns a frame. `Busy` while it is in flight or the mod has none.
    pub fn take_stream_frame(&mut self, handle: AmmHandle) -> Result<AmmCaptureStreamFrame, AmmStatus> {
        let index = self.index(handle)?;
        match self.stream.take(index) {
            Some(Ok(frame)) => Ok(AmmCaptureStreamFrame::from(frame)),
            Some(Err(failure)) => Ok(AmmCaptureStreamFrame::from(failure)),
            None => Err(AmmStatus::Busy),
        }
    }

    /// As `take_stream_frame`, but the pixels are lent until `give_back_stream_frame` with the frame's `lend_id`.
    /// See `CaptureStreamBook::lend`.
    pub fn lend_stream_frame(&mut self, handle: AmmHandle) -> Result<AmmCaptureStreamFrame, AmmStatus> {
        let index = self.index(handle)?;
        match self.stream.lend(index) {
            Some(Ok(frame)) => Ok(AmmCaptureStreamFrame::from(frame)),
            Some(Err(failure)) => Ok(AmmCaptureStreamFrame::from(failure)),
            None => Err(AmmStatus::Busy),
        }
    }

    /// Ends the lend `lend_id` of `handle`. `InvalidArgument` for an id it does not hold.
    pub fn give_back_stream_frame(&mut self, handle: AmmHandle, lend_id: u64) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.stream.give_back(index, lend_id).then_some(()).ok_or(AmmStatus::InvalidArgument)
    }

    /// Ends the capture stream of `handle`. See `CaptureStreamBook::close`.
    pub fn close_stream(&mut self, handle: AmmHandle) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.stream.close(index);
        Ok(())
    }

    /// The bytes of capture stream pixel buffers the host holds.
    pub fn stream_bytes(&self) -> usize {
        self.stream.held_bytes()
    }

    /// The buffer the overlay copies the pixels of stream copy `id` into, and its row pitch. See
    /// `CaptureStreamBook::buffer`.
    pub fn stream_buffer(&mut self, id: u32, width: u32, height: u32) -> Result<(*mut u8, u32), AmmStatus> {
        let (buffer, row_pitch) = self.stream.buffer(id, width, height).ok_or(AmmStatus::InvalidArgument)?;
        Ok((buffer.as_mut_ptr(), row_pitch))
    }

    /// The overlay copied stream copy `id` (`ok`) in the DXGI format `format`, or failed to.
    pub fn stream_done(&mut self, id: u32, ok: bool, format: u32) -> Result<(), AmmStatus> {
        self.stream.report(id, ok, format).then_some(()).ok_or(AmmStatus::InvalidArgument)
    }

    pub fn release(&mut self, handle: AmmHandle, cap: Capability) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.release(index, cap);
        Ok(())
    }

    /// Moves up to `capacity` queued events of the mod, oldest first, to `write(position, event)`.
    pub fn poll_events(
        &mut self,
        handle: AmmHandle,
        capacity: usize,
        mut write: impl FnMut(usize, AmmEvent),
    ) -> Result<usize, AmmStatus> {
        let index = self.index(handle)?;
        let mut written = 0;
        while written < capacity {
            let wanted = (capacity - written).min(self.poll_buffer.len());
            let got = self.manager.poll_events(index, &mut self.poll_buffer[..wanted]);
            for (offset, event) in self.poll_buffer[..got].iter().enumerate() {
                write(written + offset, AmmEvent::from(*event));
            }
            written += got;
            if got < wanted {
                break;
            }
        }
        Ok(written)
    }

    pub fn key_down(&self, handle: AmmHandle, vk: u32) -> Result<bool, AmmStatus> {
        let index = self.index(handle)?;
        Ok(self.manager.key_down(index, vk))
    }

    /// Whether the manager panel is open, for a live `handle`.
    pub fn panel_open(&self, handle: AmmHandle) -> Result<bool, AmmStatus> {
        self.index(handle)?;
        Ok(self.manager.snapshot().panel_open)
    }

    /// Sets the keys `handle` wants to see. See `ManagerService::watch_keys`.
    pub fn watch_keys(&mut self, handle: AmmHandle, vks: &[u32]) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.watch_keys(index, vks).map_err(|error| AmmStatus::from(&error))
    }

    /// Holds a game key down or lets it go for `handle`. See `ManagerService::send_key`.
    pub fn send_key(&mut self, handle: AmmHandle, vk: u32, down: bool) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.send_key(index, vk, down).map_err(|error| AmmStatus::from(&error))
    }

    /// Moves up to `capacity` queued fake key presses, oldest first, to `write(position, op)` and returns how many.
    /// The rest stay in `fake_ops` for the next call.
    pub fn take_fake_keys(&mut self, capacity: usize, mut write: impl FnMut(usize, FakeKeyOp)) -> usize {
        self.manager.take_fake_keys(&mut self.fake_ops);
        let count = capacity.min(self.fake_ops.len());
        for (position, op) in self.fake_ops.drain(..count).enumerate() {
            write(position, op);
        }
        count
    }

    /// Sends a fake mouse action for `handle`. See `ManagerService::send_pointer`.
    pub fn send_pointer(&mut self, handle: AmmHandle, op: PointerOp) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.send_pointer(index, op).map_err(|error| AmmStatus::from(&error))
    }

    /// Moves up to `capacity` queued fake mouse actions, oldest first, to `write(position, op)` and returns how
    /// many. The rest stay in `fake_pointer_ops` for the next call.
    pub fn take_fake_pointer(&mut self, capacity: usize, mut write: impl FnMut(usize, PointerOp)) -> usize {
        self.manager.take_fake_pointer(&mut self.fake_pointer_ops);
        let count = capacity.min(self.fake_pointer_ops.len());
        for (position, op) in self.fake_pointer_ops.drain(..count).enumerate() {
            write(position, op);
        }
        count
    }

    pub fn set_setting(&mut self, handle: AmmHandle, path: &str, value_hcl: &str) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.set_setting(index, path, value_hcl, self.now_ms).map_err(|error| AmmStatus::from(&error))
    }

    pub fn register_menu_entry(
        &mut self,
        handle: AmmHandle,
        label: &str,
        mark: &str,
        screens: MenuScreens,
    ) -> Result<u16, AmmStatus> {
        let index = self.index(handle)?;
        let id =
            self.manager.register_menu_entry(index, label, mark, screens).map_err(|error| AmmStatus::from(&error))?;
        Ok(id.0)
    }

    /// The language code, read by the mod with `handle`. That mod now gets `LocaleChanged`.
    pub fn read_locale(&mut self, handle: AmmHandle) -> Result<String, AmmStatus> {
        let index = self.index(handle)?;
        Ok(self.manager.read_locale(index))
    }

    /// The person picked another language in the panel.
    pub fn set_locale(&mut self, code: &str) {
        self.manager.set_locale(code);
        self.panel.set_locale(code);
    }

    /// Sets or clears the icon of a mod. A handle that names no live registration is `UnknownHandle`.
    pub fn set_icon(&mut self, handle: AmmHandle, icon: Option<ModIcon>) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.set_icon(index, icon).map_err(|error| AmmStatus::from(&error))
    }

    pub fn set_cursor_mode(&mut self, handle: AmmHandle, on: bool) -> Result<(), AmmStatus> {
        let index = self.index(handle)?;
        self.manager.set_cursor_mode(index, on).map_err(|error| AmmStatus::from(&error))
    }

    /// Places one input event: the manager decides, the panel toggles, and the event is kept for the next egui
    /// pass of the panel or of each mod it reaches.
    pub fn route(&mut self, ev: InputEvent, now_ms: u64) -> AmmHostRoute {
        self.now_ms = now_ms.max(self.now_ms);
        self.track(&ev);
        let decision = self.manager.route(&ev, self.now_ms);
        self.note_toggle_key(&ev, &decision);
        if decision.toggle_panel {
            self.toggle_panel();
        }
        self.keep_ui_event(&ev, &decision);
        AmmHostRoute { consume: decision.consume_from_game }
    }

    /// Composes one Present: hands on the captures the overlay reported, offers the pending ones and the pending
    /// capture stream frames, takes the size message for the game, then builds the layers.
    pub fn frame(&mut self, input: &AmmHostFrameInput) -> AmmHostFrame {
        self.now_ms = input.now_ms.max(self.now_ms);
        self.back_buffer = GamePicture::new(input.width, input.height);
        CaptureRelay::collect_written(&mut self.manager, self.io.as_ref());
        self.captures.advance(&mut self.manager, self.io.as_ref());
        let mut offered = self.captures.offer(&mut self.manager);
        let back_buffer = RenderSize::new(input.width, input.height);
        self.stream.advance(back_buffer);
        let streams =
            self.stream.offer_at(self.manager.arbiter(), back_buffer, PresentSequence::current(), &mut offered.streams);
        offered.stream_count = u32::try_from(streams).unwrap_or(0);
        offered.release_stream = self.stream.take_release();
        if self.shared.take_window_changed() {
            self.manager.render_size_window_changed();
        }
        let client = RenderSize::new(input.client_width, input.client_height);
        let message = self.manager.render_size_frame(client, self.now_ms);
        if let Some(size) = message {
            if self.last_size_message.replace(size) == Some(size) {
                // the keep-alive resend, once per second while a size is forced
                curia::debug!("render size message", { width: size.width, height: size.height });
            } else {
                curia::info!("render size message", { width: size.width, height: size.height });
            }
        }
        let Some(mut compositor) = self.compositor.take() else {
            return AmmHostFrame::empty().with_captures_of(&offered).with_size_message(message);
        };
        let composed = PanicGuard::run(
            || compositor.compose(self, input),
            |_| curia::error!("frame compose panicked, nothing drawn this frame"),
        );
        self.compositor = Some(compositor);
        composed.unwrap_or_else(AmmHostFrame::empty).with_captures_of(&offered).with_size_message(message)
    }

    /// Calls the frame callback of the mod in `index` for the layer `kind`, `allowed_layers` holding that layer's
    /// one bit, with the events routed to that layer, which are then cleared.
    pub fn call_frame(
        &mut self,
        index: ModIndex,
        input: &AmmHostFrameInput,
        (kind, allowed_layers): (LayerKind, u8),
        input_flags: u8,
    ) -> Option<(i32, AmmFrameOutput)> {
        let slot = self.slots.get_mut(usize::from(index.0))?;
        let base = FrameInputData {
            width: input.width,
            height: input.height,
            pixels_per_point: input.pixels_per_point,
            time_secs: input.time_secs,
            events: &[],
            allowed_layers,
            input_flags,
        };
        slot.run_frame(&base, kind)
    }

    /// Drops the routed events of every slot.
    pub fn clear_ui_events(&mut self) {
        for slot in self.slots.iter_mut() {
            slot.clear_events();
        }
    }

    /// Runs the panel, toast and menu button pass into `out`, records where it drew the menu button, and copies
    /// the actions it returns into `actions`.
    pub fn run_panel(&mut self, input: &AmmHostFrameInput, out: &mut MeshBuffers, actions: &mut Vec<PanelAction>) {
        let model = PanelModel::new(&self.manager, &self.manager_version, input.now_ms);
        let data = FrameInputData {
            width: input.width,
            height: input.height,
            pixels_per_point: input.pixels_per_point,
            time_secs: input.time_secs,
            events: self.panel_events.as_slice(),
            allowed_layers: FrameInputData::LAYER_OVERLAY,
            input_flags: FrameInputData::ALLOW_POINTER
                | FrameInputData::ALLOW_KEYBOARD
                | FrameInputData::ALLOW_NAV_KEYS,
        };
        actions.clear();
        actions.extend_from_slice(self.panel.run(&data, &model, out));
        self.manager.place_menu_button(self.panel.menu_button_rect());
        self.panel_events.clear();
    }

    /// Applies panel actions in order.
    pub fn apply_panel_actions(&mut self, actions: &[PanelAction]) {
        for action in actions {
            self.apply_panel_action(action);
        }
    }

    /// True while a toast is on screen at `now_ms`.
    pub fn toast_visible(&self, now_ms: u64) -> bool {
        self.manager.toasts().visible(now_ms, PanelModel::TOAST_TTL_MS).next().is_some()
    }

    /// The device dropped every texture: each mod and the panel send theirs again.
    pub fn textures_lost(&mut self) {
        for slot in self.slots.iter() {
            slot.textures_lost();
        }
        self.panel.textures_lost();
        self.captures.device_lost();
        self.stream.device_lost();
        if let Some(compositor) = self.compositor.as_mut() {
            compositor.textures_lost();
        }
    }

    /// See `ManagerService::take_focus_loss_owed`.
    pub fn take_focus_loss_owed(&mut self) -> bool {
        self.manager.take_focus_loss_owed()
    }

    /// Lets go of every fake key and mouse button the mods hold.
    pub fn release_fake_keys(&mut self) {
        self.manager.release_fake_keys();
    }

    pub fn world(&mut self, loaded: bool) {
        if loaded {
            self.manager.world_loaded();
        } else {
            self.manager.world_left();
        }
    }

    /// Hands the due config writes to the writer, gives the manager the screenshots the writer finished, ages the
    /// captures no Present has taken, logs the fake mouse actions a full queue evicted or refused since the last tick, and
    /// logs telemetry each interval when it is on.
    pub fn tick(&mut self, now_ms: u64) {
        self.now_ms = now_ms.max(self.now_ms);
        self.flush_writes(self.now_ms);
        CaptureRelay::collect_written(&mut self.manager, self.io.as_ref());
        self.manager.tick_captures();
        let pointer_drops = self.manager.take_pointer_drops();
        if pointer_drops > 0 {
            curia::warn!("fake pointer queue full, wheel actions evicted or actions refused", { dropped: pointer_drops });
        }
        let config = self.manager.config();
        let interval_ms = u64::from(config.telemetry_interval_secs) * Self::MS_PER_SEC;
        if config.telemetry && self.now_ms.saturating_sub(self.last_telemetry_ms) >= interval_ms {
            self.last_telemetry_ms = self.now_ms;
            curia::info!("telemetry", {
                report: self.manager.telemetry().report(),
                log_files_replaced: self.logs.replaced_sinks(),
            });
        }
    }

    /// Publishes what other threads read without the lock: the holder book when a holder changed, the end of the
    /// last toast, whether the menu button is placed, whether the player's turn and the gamepad are blocked, whether a capture
    /// is in flight, and the forced render size. It allocates only when a holder changed.
    pub fn publish_shared(&mut self) {
        let indices = Capability::ALL.map(|cap| self.holder_of(cap));
        if indices != self.holders.indices() {
            let manager = &self.manager;
            let book = HolderBook::new(Capability::ALL.map(|cap| {
                let index = Self::holder_in(manager, cap)?;
                let id = manager.registry().get(index)?.descriptor.id();
                Some((index, Box::from(id)))
            }));
            self.holders = Arc::new(book);
            self.shared.holders().store(Arc::clone(&self.holders));
        }
        let last = self.manager.toasts().visible(0, u64::MAX).map(|toast| toast.created_ms).max();
        self.shared.set_toast_until(last.map_or(0, |created| created.saturating_add(PanelModel::TOAST_TTL_MS)));
        self.shared.set_menu_button_placed(self.manager.menu_button_rect().is_some());
        let (blocks_turn, blocks_gamepad) = {
            let snapshot = self.shared.snapshot().load();
            (snapshot.blocks_player_turn, snapshot.blocks_gamepad)
        };
        self.shared.set_blocks_player_turn(blocks_turn);
        self.shared.set_blocks_gamepad(blocks_gamepad);
        let pending = !self.manager.pending_captures().as_slice().is_empty();
        self.shared.set_capturing(pending || self.captures.is_active() || self.stream.needs_present());
        self.shared.set_forced_size(self.manager.render_size().forced());
        self.shared.set_render_size_busy(self.manager.render_size().busy());
    }

    /// Writes what is pending within a bounded time, then closes every log file this host opened.
    pub fn shutdown(mut self) {
        self.flush_writes(u64::MAX);
        if let Some(io) = self.io.take() {
            io.shutdown(Self::IO_FLUSH);
        }
        for (slot, client) in self.slots.iter().enumerate() {
            if client.is_live()
                && let Ok(slot) = u8::try_from(slot)
            {
                self.shared.set_generation(u16::from(slot), None);
                self.logs.close(slot);
            }
        }
        curia::info!("mod manager stopped");
        self.logs.close(ModLogRouter::MANAGER_SLOT);
    }

    fn checked_config_dir(&self, config_dir: &Path) -> Result<PathBuf, AmmStatus> {
        let reason = PathRules::fault(config_dir).or_else(|| {
            let key = Self::path_key(config_dir);
            if key.starts_with(&Self::path_key(&self.manager_dir)) {
                return Some("it is the manager's own config folder or inside it");
            }
            let taken = self.mod_dirs.iter().flatten().any(|dir| Self::path_key(dir) == key);
            taken.then_some("another live mod uses it")
        });
        if let Some(reason) = reason {
            curia::warn!("mod config folder refused", { config_dir: config_dir.display().to_string(), reason: reason });
            return Err(AmmStatus::InvalidArgument);
        }
        Ok(config_dir.to_path_buf())
    }

    // The parts of a path as compared here: separators and case do not count, as on Windows.
    fn path_key(path: &Path) -> Vec<String> {
        path.components()
            .filter(|part| *part != Component::CurDir)
            .map(|part| part.as_os_str().to_string_lossy().to_lowercase())
            .collect()
    }

    fn index(&self, handle: AmmHandle) -> Result<ModIndex, AmmStatus> {
        let slot = self.slots.get(usize::from(handle.slot)).filter(|slot| slot.matches(handle));
        let index = u8::try_from(handle.slot).map_err(|_| AmmStatus::UnknownHandle)?;
        slot.map(|_| ModIndex(index)).ok_or(AmmStatus::UnknownHandle)
    }

    fn holder_of(&self, cap: Capability) -> Option<ModIndex> {
        Self::holder_in(&self.manager, cap)
    }

    fn holder_in(manager: &ManagerService, cap: Capability) -> Option<ModIndex> {
        let arbiter = manager.arbiter();
        arbiter.exclusive_holder(cap).or_else(|| arbiter.most_recent(cap).next())
    }

    // Saves the drafts whose window has passed and hands the due writes to the writer. A write the writer has no
    // room for goes back to the manager, which shows the save-failed toast and tries it again later.
    fn flush_writes(&mut self, now_ms: u64) {
        let Some(io) = self.io.as_ref() else {
            return;
        };
        let mut refused = Vec::new();
        for write in self.manager.due_writes(now_ms) {
            if io.free_slots() == 0 {
                refused.push(write);
                continue;
            }
            let path = write.path.display().to_string();
            if let Err(error) = io.submit(write) {
                curia::warn!("config write not queued", { path: path, error: error.to_string() });
            }
        }
        let now_ms = self.now_ms;
        for write in refused {
            self.manager.write_refused(write, now_ms);
        }
    }

    // Logs the first few key events (down and up, repeats included) of the toggle key with the modifiers seen and whether the chord matched.
    fn note_toggle_key(&mut self, ev: &InputEvent, decision: &RouteDecision) {
        let InputEvent::Key { vk, down, repeat, mods } = *ev else {
            return;
        };
        if vk != self.manager.config().toggle_chord.key || self.toggle_seen >= Self::TOGGLE_SEEN_LIMIT {
            return;
        }
        self.toggle_seen += 1;
        curia::debug!("toggle key seen", {
            vk: vk,
            down: down,
            repeat: repeat,
            ctrl: mods.ctrl,
            shift: mods.shift,
            alt: mods.alt,
            toggle_matched: decision.toggle_panel,
        });
    }

    // The toggle chord opens the panel at once. Closing goes through the controller, which drops a waiting rebind.
    fn toggle_panel(&mut self) {
        self.apply_toggle();
        curia::info!("panel toggled", { open: self.manager.snapshot().panel_open });
    }

    fn apply_toggle(&mut self) {
        if !self.manager.snapshot().panel_open {
            self.manager.set_panel_open(true);
            return;
        }
        let controller = self.panel.controller_mut();
        let mut actions = std::mem::take(&mut self.panel_actions);
        actions.clear();
        actions.extend(controller.on_action(&PanelAction::Close));
        self.apply_panel_actions(&actions);
        self.panel_actions = actions;
    }

    fn apply_panel_action(&mut self, action: &PanelAction) {
        let result = match action {
            PanelAction::Open => {
                self.manager.set_panel_open(true);
                curia::info!("panel opened from the menu button");
                Ok(())
            }
            PanelAction::MenuEntry { owner, id } => {
                self.manager.menu_entry_clicked(*owner, *id);
                Ok(())
            }
            PanelAction::Close => {
                self.manager.set_panel_open(false);
                self.panel.controller_mut().reset();
                Ok(())
            }
            PanelAction::SetEnabled(index, enabled) => {
                self.manager.set_enabled(*index, *enabled);
                Ok(())
            }
            PanelAction::Edit { mod_index, path, value } => {
                self.manager.edit_draft(*mod_index, path, value.clone(), self.now_ms)
            }
            PanelAction::Rebind { mod_index, action, chord } => {
                self.rebind(*mod_index, *action, ChordText::format(*chord))
            }
            PanelAction::RebindToggle(chord) => {
                self.manager.set_toggle_chord(*chord);
                Ok(())
            }
            PanelAction::SetExclusive(key, resolution) => {
                self.manager.set_exclusive_resolution(key.clone(), resolution.clone());
                Ok(())
            }
            PanelAction::SetSuspension(key, resolution) => {
                self.manager.set_suspension_resolution(key.clone(), *resolution);
                Ok(())
            }
            PanelAction::SetLanguage(code) => {
                self.set_locale(code);
                Ok(())
            }
            PanelAction::Select(_) | PanelAction::SelectTab(_) => Ok(()),
        };
        if let Err(error) = result {
            curia::warn!("panel action refused", { error: error.to_string() });
        }
    }

    fn rebind(&mut self, index: ModIndex, action: u16, chord: String) -> Result<(), ManagerError> {
        let registered = self.manager.registry().get(index).ok_or(ManagerError::UnknownMod)?;
        let bind = registered.descriptor.keybinds.get(usize::from(action)).ok_or(ManagerError::UnknownMod)?;
        let path = bind.config_path.clone();
        self.manager.edit_draft(index, &path, hcl::Value::String(chord), self.now_ms)
    }

    fn track(&mut self, ev: &InputEvent) {
        match *ev {
            InputEvent::PointerMove { x, y } | InputEvent::PointerButton { x, y, .. } => self.pointer = (x, y),
            InputEvent::Key { mods, .. } => self.modifiers = Self::modifier_mask(mods),
            _ => {}
        }
    }

    // Keeps the event for the panel when the panel takes it, and for each drawn mod it reaches. Pointer moves and
    // focus loss reach every drawn mod while the panel does not take them, so egui knows where the pointer is
    // before the mod asks for it. While the menu button shows, every move and focus loss reaches the manager's
    // pass too, so the button's hover ends when the pointer leaves it.
    fn keep_ui_event(&mut self, ev: &InputEvent, decision: &RouteDecision) {
        let Some(ui) = self.ui_event(ev) else {
            return;
        };
        let panel_open = self.manager.snapshot().panel_open;
        let button = self.manager.menu_button_rect().is_some()
            && matches!(ev, InputEvent::PointerMove { .. } | InputEvent::FocusLost);
        if decision.to_panel || button || (panel_open && *ev == InputEvent::FocusLost) {
            self.panel_events.push(ui);
        }
        let pointer = self.manager.pointer_layer();
        let snapshot = self.manager.snapshot();
        for entry in snapshot.layers.iter() {
            if self.layer_gets(entry, &ui, decision, pointer)
                && let Some(slot) = self.slots.get_mut(usize::from(entry.mod_index.0))
            {
                slot.events_mut(entry.kind).push(ui);
            }
        }
    }

    // True when the UI event goes to the layer `entry`. Pointer moves and focus loss reach every layer. A press and
    // a wheel step reach only the layer that owns the pointer, and a key down or a character only the mod's key
    // layer. A release reaches every layer of a mod it names: a release while the panel is open still names the
    // mods that got the press, so the egui of whichever layer got it lets go too.
    fn layer_gets(
        &self,
        entry: &LayerEntry,
        ui: &UiEvent,
        decision: &RouteDecision,
        pointer: Option<LayerEntry>,
    ) -> bool {
        let broadcast = matches!(ui.kind, UiEventKind::PointerMove | UiEventKind::FocusLost);
        if broadcast && !decision.to_panel {
            return true;
        }
        if !decision.raw_targets.contains(entry.mod_index) {
            return false;
        }
        match ui.kind {
            UiEventKind::PointerMove | UiEventKind::FocusLost => true,
            UiEventKind::PointerButton | UiEventKind::Key if !ui.down => true,
            UiEventKind::PointerButton | UiEventKind::Wheel => pointer == Some(*entry),
            _ => self.manager.key_layer(entry.mod_index) == Some(entry.kind),
        }
    }

    fn ui_event(&self, ev: &InputEvent) -> Option<UiEvent> {
        let (x, y) = self.pointer;
        let base =
            UiEvent { kind: UiEventKind::PointerMove, x, y, key: 0, modifiers: self.modifiers, down: false, ch: 0 };
        let event = match *ev {
            InputEvent::Key { vk, down, .. } => UiEvent { kind: UiEventKind::Key, key: vk, down, ..base },
            InputEvent::PointerMove { x, y } => UiEvent { x, y, ..base },
            InputEvent::PointerButton { button, down, x, y } => {
                UiEvent { kind: UiEventKind::PointerButton, key: u32::from(button), down, x, y, ..base }
            }
            InputEvent::Wheel { delta } => UiEvent { kind: UiEventKind::Wheel, x: 0.0, y: delta, ..base },
            InputEvent::Char(ch) => UiEvent { kind: UiEventKind::Char, ch, ..base },
            InputEvent::FocusLost => UiEvent { kind: UiEventKind::FocusLost, ..base },
            InputEvent::PointerDelta { .. } => return None,
        };
        Some(event)
    }

    fn modifier_mask(mods: Modifiers) -> u8 {
        let mut mask = 0;
        if mods.ctrl {
            mask |= UiEvent::MOD_CTRL;
        }
        if mods.shift {
            mask |= UiEvent::MOD_SHIFT;
        }
        if mods.alt {
            mask |= UiEvent::MOD_ALT;
        }
        mask
    }

    // The manager config, its file for later writes, and the reason it could not be read, if any. A missing file
    // gives the defaults silently and is created on the first change; a file that does not read is never written,
    // so a hand edit with a mistake is not lost.
    fn read_config(config_dir: &Path) -> (ManagerConfig, Option<(PathBuf, HclDocument)>, Option<String>) {
        let path = config_dir.join(Self::CONFIG_FILE);
        match fs::read_to_string(&path) {
            Ok(text) => match ManagerConfigFile::read(&text) {
                Ok((config, document)) => (config, Some((path, document)), None),
                Err(error) => (ManagerConfig::default(), None, Some(error)),
            },
            Err(error) if error.kind() == ErrorKind::NotFound => {
                (ManagerConfig::default(), Some((path, HclDocument::empty())), None)
            }
            Err(error) => (ManagerConfig::default(), None, Some(error.to_string())),
        }
    }

    // Reads the mod's config file and logs a file that is replaced by defaults.
    fn read_document(path: &Path) -> ReadOutcome {
        let outcome = ConfigFile::read(path);
        match outcome {
            ReadOutcome::Invalid => {
                curia::warn!("mod config is not valid HCL, kept as .invalid and replaced by defaults", {
                    path: path.display().to_string(),
                })
            }
            ReadOutcome::Unreadable => {
                curia::warn!("mod config could not be read or kept, defaults used and the file is not written", {
                    path: path.display().to_string(),
                })
            }
            ReadOutcome::Parsed(_) | ReadOutcome::Missing => {}
        }
        outcome
    }
}
