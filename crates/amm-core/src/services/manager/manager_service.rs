use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use hcl::eval::{Context, Evaluate};

use crate::config::{ExclusiveResolution, ManagerConfig, PairKey, SuspensionResolution};
use crate::errors::{CaptureError, ManagerError};
use crate::services::capture::CaptureBook;
use crate::services::clash::ClashAnalyzer;
use crate::services::descriptor::DescriptorChecker;
use crate::services::event::EventHub;
use crate::services::icon::IconBook;
use crate::services::input::{
    ChordMatcher, CursorSet, FakeKeys, FakePointer, GameViewportBook, InputRouter, KeyState, KeyWatch,
};
use crate::services::lease::LeaseArbiter;
use crate::services::menu::MenuEntries;
use crate::services::registry::ModRegistry;
use crate::services::render::RenderSizeBook;
use crate::services::settings::{DraftBook, EffectiveKeys, ManagerConfigFile, SchemaChecker};
use crate::services::snapshot::SnapshotBuilder;
use crate::services::telemetry::Telemetry;
use crate::services::toast::ToastQueue;
use crate::structs::capture::{PendingCaptures, ScreenshotLayers};
use crate::structs::clash::ClashReport;
use crate::structs::descriptor::{ApplyMode, Capability, Chord, Descriptor, FieldKind, SettingField};
use crate::structs::event::{KeybindPhase, ModEvent};
use crate::structs::frame::{FrameReplyData, LayerKind, MenuScreen};
use crate::structs::icon::{IconEntry, ModIcon};
use crate::structs::input::{
    FakeKeyOp, GameViewport, InputEvent, PointerButton, PointerOp, RouteContext, RouteDecision,
};
use crate::structs::lease::AcquireOutcome;
use crate::structs::manager::{ManagedMod, TextureGrant};
use crate::structs::menu::{MenuEntryId, MenuScreens};
use crate::structs::registry::{ModIndex, ModMask};
use crate::structs::render::{RenderSize, RenderSizeState};
use crate::structs::settings::{CheckedConfig, HclDocument, SettingsDraft};
use crate::structs::snapshot::{LayerEntry, RenderSnapshot};
use crate::structs::toast::ToastKind;
use crate::structs::write::{SaveStatus, WriteRequest};

use super::{LayerReplies, PanelState, SaveQueue, TextureGranter};

/// The manager's game-thread state and every operation the ABI table exposes. It owns the
/// registry, leases, events, toasts, routing, settings and the write-merge state, and publishes a
/// `RenderSnapshot` through an `ArcSwap` after each change that the render thread must see.
///
/// Saving is automatic: an operator edit waits in its mod's draft until one merge window has passed
/// since that mod's last edit, then it is checked, applied and queued as one write.
///
/// Each mod gets its texture budget at registration, from its descriptor (`TextureGranter`).
///
/// Bounds: one `ManagedMod` and one `SettingsDraft` per mod slot, so both are bounded by the mod
/// limit. The write table holds two entries per mod slot; a write it refuses shows the save-failed
/// toast. The manager does no file I/O: it hands out `WriteRequest` values.
pub struct ManagerService {
    config: ManagerConfig,
    checker: DescriptorChecker,
    granter: TextureGranter,
    registry: ModRegistry,
    arbiter: LeaseArbiter,
    hub: EventHub,
    toasts: ToastQueue,
    analyzer: ClashAnalyzer,
    keys: EffectiveKeys,
    matcher: ChordMatcher,
    router: InputRouter,
    key_state: KeyState,
    fake_keys: FakeKeys,
    fake_pointer: FakePointer,
    key_watch: KeyWatch,
    /// The mods that held an active `Input` lease at the last publish.
    input_holders: ModMask,
    /// The active exclusive `Input` holder with `full_block` at the last publish.
    full_block_holder: Option<ModIndex>,
    /// A focus loss came while a full-block holder was active: the holder kept its keys, and the full release runs
    /// when its full block ends.
    focus_lost_deferred: bool,
    /// A full block ended after it kept a focus loss from the game, and the game has not seen a focus loss since.
    focus_loss_owed: bool,
    cursor: CursorSet,
    viewports: GameViewportBook,
    render_size: RenderSizeBook,
    panel: PanelState,
    menu: MenuEntries,
    icons: IconBook,
    /// The mods that have read the language code. Only these get `LocaleChanged`: a mod built for an ABI older
    /// than 1.5 never reads it, and cannot decode that event kind.
    locale_readers: ModMask,
    replies: LayerReplies,
    captures: CaptureBook,
    mods: Vec<Option<ManagedMod>>,
    drafts: DraftBook,
    saves: SaveQueue,
    next_seq: u64,
    snapshot: Arc<ArcSwap<RenderSnapshot>>,
    telemetry: Arc<Telemetry>,
}

impl ManagerService {
    /// How long the save status says "Saved" after a write goes out.
    pub const SAVED_SHOW_MS: u64 = SaveQueue::SAVED_SHOW_MS;

    /// `config_file` is the path and document of the manager's own `config.hcl`. With it, a change of an enabled
    /// flag or a resolution is written back there through the merge window; without it the change stays in memory.
    pub fn new(config: ManagerConfig, config_file: Option<(PathBuf, HclDocument)>) -> Self {
        let slots = config.budgets.max_mods.min(ModMask::CAPACITY);
        let budgets = &config.budgets;
        let checker = DescriptorChecker::new(budgets.max_keybinds, budgets.max_fields);
        let hub = EventHub::new(slots, budgets.event_queue);
        let toasts = ToastQueue::new(budgets.toasts, budgets.toast_rate_ms);
        let snapshot = RenderSnapshot { menu_button: config.menu_button, ..RenderSnapshot::new() };
        Self {
            checker,
            granter: TextureGranter::new(budgets),
            registry: ModRegistry::new(slots),
            arbiter: LeaseArbiter::new(),
            hub,
            toasts,
            analyzer: ClashAnalyzer::new(),
            keys: EffectiveKeys::new(),
            matcher: ChordMatcher::new(EffectiveKeys::CAPACITY),
            router: InputRouter::new(slots),
            key_state: KeyState::new(),
            fake_keys: FakeKeys::new(),
            fake_pointer: FakePointer::new(),
            key_watch: KeyWatch::new(),
            input_holders: ModMask::default(),
            full_block_holder: None,
            focus_lost_deferred: false,
            focus_loss_owed: false,
            cursor: CursorSet::new(),
            viewports: GameViewportBook::new(),
            render_size: RenderSizeBook::new(),
            panel: PanelState::new(config.menu_button),
            menu: MenuEntries::new(),
            icons: IconBook::new(slots),
            locale_readers: ModMask::default(),
            replies: LayerReplies::new(slots),
            captures: CaptureBook::new(),
            mods: (0..slots).map(|_| None).collect(),
            drafts: DraftBook::new(slots, budgets.write_merge_ms),
            saves: SaveQueue::new(config_file, slots.max(1) * 2, budgets.write_merge_ms),
            next_seq: 0,
            snapshot: Arc::new(ArcSwap::from_pointee(snapshot)),
            telemetry: Telemetry::new_shared(),
            config,
        }
    }

    /// Registers a mod. `document` is its config file, or `None` when there is no file. The file is
    /// checked against the schema, and a write is queued when the checked text differs from the
    /// given text. The config path is the descriptor's `config_path` inside `config_dir`. A mod that
    /// the manager config disables still registers and gets `Disabled` as its first event.
    pub fn register(
        &mut self,
        descriptor_hcl: &str,
        document: Option<HclDocument>,
        config_dir: &Path,
    ) -> Result<(ModIndex, CheckedConfig), ManagerError> {
        self.register_with(descriptor_hcl, document, config_dir, true)
    }

    /// Registers a mod whose config file exists but could not be read or kept. It runs on the schema defaults in
    /// memory, and the mod is marked do-not-write: no write is queued at registration or later by `set_setting` or
    /// a panel edit, and each refused change shows the save-failed toast, so the file on disk is left as it is.
    pub fn register_unwritten(
        &mut self,
        descriptor_hcl: &str,
        config_dir: &Path,
    ) -> Result<(ModIndex, CheckedConfig), ManagerError> {
        self.register_with(descriptor_hcl, None, config_dir, false)
    }

    fn register_with(
        &mut self,
        descriptor_hcl: &str,
        document: Option<HclDocument>,
        config_dir: &Path,
        write_back: bool,
    ) -> Result<(ModIndex, CheckedConfig), ManagerError> {
        let descriptor = self.checker.parse(descriptor_hcl)?;
        let enabled = self.config.is_enabled(&descriptor.id());
        let original = document.as_ref().map(HclDocument::text);
        let checked = SchemaChecker::check(&descriptor.settings, document.unwrap_or_else(HclDocument::empty));
        let config_path = config_dir.join(&descriptor.config_path);
        let index = self.registry.register(descriptor, enabled)?;
        let grant = self
            .registry
            .get(index)
            .map_or_else(|| self.granter.default_grant(), |registered| self.granter.grant(&registered.descriptor));
        let text = checked.document.text();
        if write_back && original.as_deref() != Some(text.as_str()) {
            self.queue_write(config_path.clone(), text, 0);
        }
        self.next_seq += 1;
        let slot = usize::from(index.0);
        self.drafts.reset(slot, checked.document.clone());
        self.mods[slot] =
            Some(ManagedMod::new(config_path, checked.document.clone(), self.next_seq, write_back, grant));
        self.hub.clear(index);
        if !enabled {
            self.hub.push(index, ModEvent::Disabled);
        }
        self.refresh();
        Ok((index, checked))
    }

    /// Sets or clears the icon of the mod in `index`. A slot with no mod is `UnknownMod`.
    pub fn set_icon(&mut self, index: ModIndex, icon: Option<ModIcon>) -> Result<(), ManagerError> {
        self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        self.icons.set(index, icon);
        Ok(())
    }

    /// The icon of the mod in `index`, if it set one.
    pub fn icon(&self, index: ModIndex) -> Option<&IconEntry> {
        self.icons.get(index)
    }

    /// Removes a mod. Its leases are released first, so the other mods get their events.
    pub fn unregister(&mut self, index: ModIndex) {
        if self.registry.get(index).is_none() {
            return;
        }
        self.release_fake_mod(index);
        self.key_watch.forget(index);
        self.arbiter.release_all(index, &mut self.hub);
        self.captures.forget(index);
        self.menu.forget(index);
        self.icons.forget(index);
        self.locale_readers.remove(index);
        self.cursor.remove(index);
        self.viewports.set(index, None);
        self.registry.unregister(index);
        self.router.forget(index);
        self.hub.clear(index);
        self.forget_reply(index);
        let slot = usize::from(index.0);
        self.mods[slot] = None;
        self.drafts.reset(slot, HclDocument::empty());
        self.telemetry.clear_mod(index);
        self.refresh();
    }

    /// Asks for a lease. A disabled mod holds no leases, so its request is `NotPermitted`. A denial
    /// pushes the denial toast, within its rate limit.
    pub fn acquire(&mut self, index: ModIndex, cap: Capability, now_ms: u64) -> Result<AcquireOutcome, ManagerError> {
        let registered = self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        if !registered.enabled {
            return Err(ManagerError::NotPermitted);
        }
        let outcome = self.arbiter.acquire(&self.registry, &self.config.resolutions, index, cap, &mut self.hub);
        if let AcquireOutcome::Denied { holder } = outcome
            && holder != index
        {
            self.toasts.push(ToastKind::Denied { capability: cap, holder, requester: index }, now_ms);
        }
        self.publish();
        Ok(outcome)
    }

    pub fn release(&mut self, index: ModIndex, cap: Capability) {
        if self.registry.get(index).is_none() {
            return;
        }
        self.arbiter.release(index, cap, &mut self.hub);
        self.publish();
    }

    /// Books a screenshot of the next Present at the capture point `layers`, written to `path` as PNG, and returns
    /// its sequence. See `CaptureBook::request` for the rules.
    pub fn request_capture(
        &mut self,
        index: ModIndex,
        path: &Path,
        layers: ScreenshotLayers,
    ) -> Result<u32, CaptureError> {
        self.registry.get(index).ok_or(CaptureError::UnknownMod)?;
        self.captures.request(&self.arbiter, index, path, layers)
    }

    /// True when the mod in `index` may ask for a capture: it holds an active `Hud` or `Overlay` lease.
    pub fn may_capture(&self, index: ModIndex) -> bool {
        CaptureBook::may_capture(&self.arbiter, index)
    }

    /// The captures that wait for the overlay. It does not allocate.
    pub fn pending_captures(&self) -> PendingCaptures {
        self.captures.pending()
    }

    /// The captures that wait for the overlay, marked taken. It does not allocate.
    pub fn take_captures(&mut self) -> PendingCaptures {
        self.captures.take()
    }

    /// See `CaptureBook::taken`. Returns true when the capture now waits for its write to `capture_path`.
    pub fn capture_taken(&mut self, sequence: u32, ok: bool) -> bool {
        self.captures.taken(sequence, ok, &mut self.hub)
    }

    /// Ages the pending captures by one manager tick and fails the ones the host has not taken in time. See
    /// `CaptureBook::tick`.
    pub fn tick_captures(&mut self) {
        self.captures.tick(&mut self.hub);
    }

    pub fn capture_written(&mut self, sequence: u32, ok: bool) {
        self.captures.written(sequence, ok, &mut self.hub);
    }

    pub fn capture_path(&self, sequence: u32) -> Option<&Path> {
        self.captures.path(sequence)
    }

    /// True when a mod other than `index` holds an active lease on `cap`. Through the table it is answered on the
    /// game thread; from a frame callback the host is busy and the answer is false.
    pub fn held_by_other(&self, index: ModIndex, cap: Capability) -> bool {
        self.arbiter.held_by_other(index, cap)
    }

    /// The texture budget the mod got at registration, while it is registered.
    pub fn texture_grant(&self, index: ModIndex) -> Option<TextureGrant> {
        self.mods.get(usize::from(index.0))?.as_ref().map(|managed| managed.texture_grant)
    }

    /// Moves the oldest queued events of a mod into `out` and returns how many were moved.
    pub fn poll_events(&mut self, index: ModIndex, out: &mut [ModEvent]) -> usize {
        let queue = self.hub.queue_mut(index);
        let mut count = 0;
        for slot in out.iter_mut() {
            let Some(event) = queue.pop() else {
                break;
            };
            *slot = event;
            count += 1;
        }
        count
    }

    /// True when the key is down and the mod holds an active `Input` lease.
    pub fn key_down(&self, index: ModIndex, vk: u32) -> bool {
        self.arbiter.holds(index, Capability::Input) && self.key_state.is_down(vk)
    }

    /// A value a mod changed in play, as one HCL expression. The saved document and the draft take
    /// it (an operator edit of the same path keeps priority in the draft) and a merged write is
    /// queued. The mod that made the change gets no `SettingsChanged`.
    pub fn set_setting(
        &mut self,
        index: ModIndex,
        path: &str,
        value_hcl: &str,
        now_ms: u64,
    ) -> Result<(), ManagerError> {
        let registered = self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        let field = Self::field(&registered.descriptor, path).ok_or_else(|| Self::setting_error(path))?;
        let value = Self::parse_value(value_hcl).ok_or_else(|| Self::setting_error(path))?;
        let value = SchemaChecker::check_value(field, &value).map_err(|_| Self::setting_error(path))?;
        let is_chord = matches!(field.kind, FieldKind::Chord);
        let slot = usize::from(index.0);
        let managed = self.mods.get_mut(slot).and_then(Option::as_mut).ok_or(ManagerError::UnknownMod)?;
        managed.document = managed.document.with(path, &value);
        let (config_path, text) = (managed.config_path.clone(), managed.document.text());
        self.drafts.apply_game_change(slot, path, value);
        self.queue_mod_write(slot, config_path, text, now_ms);
        if is_chord {
            self.refresh();
        }
        Ok(())
    }

    /// Frees the cursor for a mod or gives it back. Asking for a free cursor needs an active
    /// `Input` or `Overlay` lease; giving it back is always allowed.
    pub fn set_cursor_mode(&mut self, index: ModIndex, on: bool) -> Result<(), ManagerError> {
        if self.registry.get(index).is_none() {
            return Err(ManagerError::UnknownMod);
        }
        let holds = self.arbiter.holds(index, Capability::Input) || self.arbiter.holds(index, Capability::Overlay);
        if on && !holds {
            return Err(ManagerError::NotPermitted);
        }
        self.cursor.request(index, on);
        self.publish();
        Ok(())
    }

    /// The mod with a focused text field: the one whose last frame reported `wants_keyboard`.
    pub fn keyboard_owner(&self) -> Option<ModIndex> {
        self.replies.keyboard_owner(&self.snapshot.load().layers)
    }

    /// Places one input event, pushes its keybind and raw events to the mods and its blocked toast
    /// to the toast queue, and returns the decision for the caller (cancel the game event, toggle or
    /// feed the panel). On focus loss every held key is released to the mods that got its down
    /// before the key state is cleared, so no key stays down in a mod. While a full-block holder is active, a focus
    /// loss is kept from the game and the holder keeps its keys; see `defer_focus_loss`.
    pub fn route(&mut self, ev: &InputEvent, now_ms: u64) -> RouteDecision {
        let started = self.config.telemetry.then(Instant::now);
        let holder = if *ev == InputEvent::FocusLost { self.full_block_holder_now() } else { None };
        let decision = match holder {
            Some(holder) => self.defer_focus_loss(holder),
            None => self.route_event(ev, now_ms),
        };
        self.telemetry.record_event_drops(self.hub.dropped());
        if let Some(started) = started {
            self.telemetry.record_route(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
        }
        decision
    }

    fn route_event(&mut self, ev: &InputEvent, now_ms: u64) -> RouteDecision {
        if *ev == InputEvent::FocusLost {
            self.focus_lost_deferred = false;
            self.focus_loss_owed = false;
            self.release_held(None);
        }
        self.panel.track(ev);
        let ctx = RouteContext {
            panel_open: self.panel.is_open(),
            toggle_chord: self.config.toggle_chord,
            pointer_owner: self.pointer_layer().map(|entry| entry.mod_index),
            keyboard_owner: self.keyboard_owner(),
            over_menu_button: self.panel.over_menu_button(),
            over_game_viewport: self.game_viewport_at(self.panel.pointer()).is_some(),
            registry: &self.registry,
            arbiter: &self.arbiter,
            book: &self.config.resolutions,
        };
        let mut decision = self.router.route(ev, &ctx, &self.matcher);
        if let InputEvent::Key { vk, down: false, .. } = *ev
            && self.fake_keys.is_held(vk)
        {
            decision.consume_from_game = true;
        }
        self.key_state.apply(ev);
        if *ev == InputEvent::FocusLost {
            self.release_fake_all();
        }
        self.deliver(ev, &decision, now_ms);
        decision
    }

    /// A focus loss while `holder` has an active full block. Every other mod gets the releases of its held keys
    /// and keybinds and loses its fake keys, as on any focus loss. The holder keeps its held keys, keybinds and
    /// fake keys, and the game does not see the focus loss. The full release runs when the holder's full block
    /// ends.
    fn defer_focus_loss(&mut self, holder: ModIndex) -> RouteDecision {
        self.release_held(Some(holder));
        self.release_fake_except(holder);
        self.focus_lost_deferred = true;
        RouteDecision { consume_from_game: true, ..RouteDecision::default() }
    }

    /// Lets go of the fake keys and buttons of every mod except `keep`. Each mod that lost one gets
    /// `FakeKeysReleased`. Every up is queued: the window has lost focus, so no real key-up will reach the game.
    fn release_fake_except(&mut self, keep: ModIndex) {
        let mut lost = self.fake_keys.release_all_except(keep);
        lost.0 |= self.fake_pointer.release_all_except(keep).0;
        for index in lost.iter() {
            self.hub.push(index, ModEvent::FakeKeysReleased);
        }
    }

    /// True once after a full block ended that had kept a focus loss from the game, unless the game has seen a
    /// focus loss since. The caller then lets the game see one focus loss, so the game lets go of its own state
    /// (for example its cursor clip) while the window is in the background.
    pub fn take_focus_loss_owed(&mut self) -> bool {
        std::mem::take(&mut self.focus_loss_owed)
    }

    /// Sets or clears (`None`) where the mod draws the game picture. It counts only while the mod holds `Overlay`.
    pub fn set_game_viewport(&mut self, index: ModIndex, viewport: Option<GameViewport>) {
        self.viewports.set(index, viewport);
    }

    /// The game viewport under `point` of the mod that holds `Overlay`, while that mod's layer is the one under
    /// `point`: a layer of another mod drawn over the picture keeps the point from the game.
    pub fn game_viewport_at(&self, point: (f32, f32)) -> Option<GameViewport> {
        let (owner, viewport) = self.viewports.at(point, &self.arbiter)?;
        let under = self.replies.pointer_layer(&self.snapshot.load().layers, point)?;
        (under.mod_index == owner).then_some(viewport)
    }

    /// Forces the game to draw at `size`, or ends the size the mod forces with `None`. The mod must hold
    /// `RenderSize`, else `NotPermitted`; a side of 0 or above `RenderSize::MAX_SIDE` is `RenderSizeOutOfRange`.
    /// The size ends on its own when the mod no longer holds the lease. See `RenderSizeBook`.
    pub fn set_render_size(&mut self, index: ModIndex, size: Option<RenderSize>) -> Result<(), ManagerError> {
        self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        if !self.arbiter.holds(index, Capability::RenderSize) {
            return Err(ManagerError::NotPermitted);
        }
        self.render_size.set(index, size)
    }

    /// What the mod reads back about the forced size, with the back buffer `actual` of the last Present in DXGI
    /// `format`. `NotPermitted` while the mod does not hold `RenderSize`.
    pub fn render_size_state(
        &self,
        index: ModIndex,
        actual: RenderSize,
        format: u32,
    ) -> Result<RenderSizeState, ManagerError> {
        self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        if !self.arbiter.holds(index, Capability::RenderSize) {
            return Err(ManagerError::NotPermitted);
        }
        Ok(self.render_size.state(index, actual, format))
    }

    /// One frame at `now_ms` with the window client size `client`: the size message to send the game now, if any.
    pub fn render_size_frame(&mut self, client: RenderSize, now_ms: u64) -> Option<RenderSize> {
        self.render_size.frame(client, now_ms)
    }

    /// The game window had a DPI, display or position change. See `RenderSizeBook::window_changed`.
    pub fn render_size_window_changed(&mut self) {
        self.render_size.window_changed();
    }

    pub fn render_size(&self) -> &RenderSizeBook {
        &self.render_size
    }

    /// The manager stops: the size message that gives the game its own `client` size back, if one is due.
    pub fn render_size_shutdown(&mut self, client: RenderSize) -> Option<RenderSize> {
        self.render_size.shutdown(client)
    }

    /// The manager starts again on a host that stayed alive: the render size book forces sizes again.
    pub fn render_size_open(&mut self) {
        self.render_size.open();
    }

    /// Enables or disables a mod and stores the choice in the manager config. Disabling releases
    /// every lease and sends `Disabled`; enabling sends `Enabled` and resets the frame failures.
    pub fn set_enabled(&mut self, index: ModIndex, enabled: bool) {
        let Some(registered) = self.registry.get(index) else {
            return;
        };
        if registered.enabled == enabled {
            return;
        }
        let id = registered.descriptor.id();
        if !enabled {
            self.release_fake_mod(index);
            self.key_watch.forget(index);
            self.arbiter.release_all(index, &mut self.hub);
            self.cursor.remove(index);
            self.forget_reply(index);
        }
        self.registry.set_enabled(index, enabled);
        self.write_config(|document| ManagerConfigFile::with_enabled(document, &id, enabled));
        self.config.enabled.insert(id, enabled);
        self.hub.push(index, if enabled { ModEvent::Enabled } else { ModEvent::Disabled });
        if enabled {
            self.clear_failures(index);
        }
        self.refresh();
    }

    pub fn set_suspension_resolution(&mut self, key: PairKey, resolution: SuspensionResolution) {
        self.write_config(|document| ManagerConfigFile::with_suspension(document, &key, resolution));
        self.config.resolutions = self.config.resolutions.with_suspension(key, resolution);
        self.refresh();
    }

    pub fn set_exclusive_resolution(&mut self, key: PairKey, resolution: ExclusiveResolution) {
        self.write_config(|document| ManagerConfigFile::with_exclusive(document, &key, &resolution));
        self.config.resolutions = self.config.resolutions.with_exclusive(key, resolution);
        self.refresh();
    }

    /// Opens or closes the panel. Opening it releases every fake key and button.
    pub fn set_panel_open(&mut self, open: bool) {
        if self.panel.set_open(open) {
            if open {
                self.release_fake_all();
            }
            self.publish();
        }
    }

    /// The game screen on top in the frame the host is about to draw. The menu button shows only on
    /// the start and the pause screen.
    pub fn set_menu_screen(&mut self, screen: MenuScreen) {
        self.panel.set_menu_screen(screen);
    }

    /// A new chord for the panel toggle. It routes at once and is written to the manager config as
    /// `toggle_chord` through the merge window.
    pub fn set_toggle_chord(&mut self, chord: Chord) {
        self.write_config(|document| ManagerConfigFile::with_toggle_chord(document, chord));
        self.config.toggle_chord = chord;
        self.publish();
    }

    /// The language code for every mod, as `config.hcl` holds it: empty for English.
    pub fn locale(&self) -> &str {
        &self.config.language
    }

    /// The language code, read by the mod in `index`. That mod now gets `LocaleChanged` when the code changes.
    pub fn read_locale(&mut self, index: ModIndex) -> String {
        if self.registry.get(index).is_some() {
            self.locale_readers.insert(index);
        }
        self.config.language.clone()
    }

    /// A new language code from the panel. It is written to the manager config as `language` through the merge
    /// window, and each mod that has read the code gets `LocaleChanged`. The same code again does nothing.
    pub fn set_locale(&mut self, code: &str) {
        if self.config.language == code {
            return;
        }
        self.write_config(|document| ManagerConfigFile::with_language(document, code));
        self.config.language = code.to_owned();
        for index in self.locale_readers.iter() {
            self.hub.push(index, ModEvent::LocaleChanged);
        }
    }

    pub fn menu_button_visible(&self) -> bool {
        self.panel.menu_button_visible()
    }

    /// Where the manager's pass drew the menu button, in physical pixels; routing gives the button the
    /// pointer inside it.
    pub fn place_menu_button(&mut self, rect: Option<[f32; 4]>) {
        self.panel.place_menu_button(rect);
    }

    /// Places a menu entry of the mod in `index` in the corner row, on `screens`. Returns its id.
    pub fn register_menu_entry(
        &mut self,
        index: ModIndex,
        label: &str,
        mark: &str,
        screens: MenuScreens,
    ) -> Result<MenuEntryId, ManagerError> {
        if self.registry.get(index).is_none() {
            return Err(ManagerError::UnknownMod);
        }
        self.menu.add(index, label, mark, screens)
    }

    /// The person clicked entry `id` of the mod in `index`: that mod gets `MenuEntryClicked`. An entry the mod no
    /// longer has sends nothing.
    pub fn menu_entry_clicked(&mut self, index: ModIndex, id: MenuEntryId) {
        if self.menu.contains(index, id) {
            self.hub.push(index, ModEvent::MenuEntryClicked { entry: id.0 });
        }
    }

    pub fn menu_entries(&self) -> &MenuEntries {
        &self.menu
    }

    /// The game screen on top, as the last `set_menu_screen` gave it.
    pub fn menu_screen(&self) -> MenuScreen {
        self.panel.menu_screen()
    }

    /// The menu button's rect while it shows.
    pub fn menu_button_rect(&self) -> Option<[f32; 4]> {
        self.panel.menu_button_rect()
    }

    /// The snapshot the render thread draws from. Reading it does not allocate.
    pub fn snapshot(&self) -> Arc<RenderSnapshot> {
        self.snapshot.load_full()
    }

    /// The swap the snapshot is published through, for a reader on another thread.
    pub fn snapshot_source(&self) -> Arc<ArcSwap<RenderSnapshot>> {
        Arc::clone(&self.snapshot)
    }

    pub fn clash_report(&self) -> &ClashReport {
        self.analyzer.report()
    }

    /// The settings drafts, indexed by mod slot. A free slot holds an empty draft.
    pub fn draft(&self) -> &[SettingsDraft] {
        self.drafts.drafts()
    }

    /// An operator edit in the panel. The value must fit the schema field at `path`. It is saved once
    /// a merge window has passed since the mod's last edit.
    pub fn edit_draft(
        &mut self,
        index: ModIndex,
        path: &str,
        value: hcl::Value,
        now_ms: u64,
    ) -> Result<(), ManagerError> {
        let registered = self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        let field = Self::field(&registered.descriptor, path).ok_or_else(|| Self::setting_error(path))?;
        let value = SchemaChecker::check_value(field, &value).map_err(|_| Self::setting_error(path))?;
        self.drafts.edit(usize::from(index.0), path, value, now_ms);
        Ok(())
    }

    /// What the panel header says about saving at `now_ms`: a refused write until the next write goes
    /// out, an edit or a write still waiting, a write handed out in the last `SAVED_SHOW_MS`, or
    /// nothing.
    pub fn save_status(&self, now_ms: u64) -> SaveStatus {
        self.saves.status(self.drafts.is_pending(), now_ms)
    }

    /// Counts one failed frame callback. At the failure limit in a row the mod is marked failed:
    /// its leases are released, its cursor request is removed, its layer is dropped and a toast is
    /// shown. It keeps its registration and settings. Returns true while the mod is failed.
    pub fn record_frame_failure(&mut self, index: ModIndex, now_ms: u64) -> bool {
        let limit = self.config.budgets.frame_failure_limit;
        let Some(registered) = self.registry.get(index) else {
            return false;
        };
        if registered.failed {
            return true;
        }
        let Some(managed) = self.mods.get_mut(usize::from(index.0)).and_then(Option::as_mut) else {
            return false;
        };
        managed.failures = managed.failures.saturating_add(1);
        let failures = managed.failures;
        if failures < limit {
            return false;
        }
        curia::error!("frame callback failed too often, mod no longer drawn", {
            mod_id: registered.descriptor.id().as_str(),
            failures: failures,
        });
        self.registry.set_failed(index, true);
        self.arbiter.release_all(index, &mut self.hub);
        self.cursor.remove(index);
        self.forget_reply(index);
        self.toasts.push(ToastKind::FrameDisabled(index), now_ms);
        self.publish();
        true
    }

    /// A frame callback that succeeded ends the run of failures. It does not allocate or publish.
    pub fn record_frame_success(&mut self, index: ModIndex) {
        if let Some(managed) = self.mods.get_mut(usize::from(index.0)).and_then(Option::as_mut) {
            managed.failures = 0;
        }
    }

    /// What the last frame of one layer of a mod reported. Routing reads it: the mod of the top-most
    /// layer under the pointer whose reply wants the pointer gets pointer events, and the mod that
    /// last reported `wants_keyboard` on a layer gets key and text events until no layer of it does.
    /// It does not allocate or publish.
    pub fn record_frame_reply(&mut self, index: ModIndex, kind: LayerKind, reply: FrameReplyData) {
        self.replies.record(index, kind, reply);
    }

    /// Tells the manager whether a mod Overlay layer covers the whole window of `size` (width, height in physical
    /// pixels), from the last replies. The corner row hides while one does. It does not allocate or publish.
    pub fn update_overlay_covers(&mut self, size: (u32, u32)) {
        let covers = self.replies.overlay_covers(&self.snapshot.load().layers, size);
        self.panel.set_overlay_covers(covers);
    }

    /// The drawn layer that owns the pointer now: the top-most layer under it whose last reply wants it.
    pub fn pointer_layer(&self) -> Option<LayerEntry> {
        self.replies.pointer_layer(&self.snapshot.load().layers, self.panel.pointer())
    }

    /// The layer of the mod in `index` that gets its key and text events: the drawn layer that wants the keyboard,
    /// the Overlay layer when both do, else the Overlay layer, else the Hud layer.
    pub fn key_layer(&self, index: ModIndex) -> Option<LayerKind> {
        self.replies.key_layer(&self.snapshot.load().layers, index)
    }

    /// Clears the failures of a mod. A failed mod is drawn again once it holds a lease.
    pub fn reset_failures(&mut self, index: ModIndex) {
        let was_failed = self.registry.get(index).is_some_and(|registered| registered.failed);
        self.clear_failures(index);
        if was_failed {
            self.publish();
        }
    }

    /// The world was left: every fake key and button is released.
    pub fn world_left(&mut self) {
        self.release_fake_keys();
    }

    /// Lets go of every fake key and button. Each mod that lost one gets `FakeKeysReleased`.
    pub fn release_fake_keys(&mut self) {
        self.release_fake_all();
    }

    /// Sets the keys the mod sees as `Key` events whatever else they do, unless a keybind or the panel took them.
    /// It replaces the earlier set; an empty slice clears it. The mod needs an `Input` claim, else `NotPermitted`.
    /// A code of 256 or more, or more than `KeyWatch::MAX_KEYS` keys, is `InvalidArgument`.
    pub fn watch_keys(&mut self, index: ModIndex, vks: &[u32]) -> Result<(), ManagerError> {
        self.require_input_claim(index)?;
        if vks.len() > KeyWatch::MAX_KEYS || vks.iter().any(|vk| *vk >= InputEvent::VK_SLOTS as u32) {
            return Err(ManagerError::InvalidArgument);
        }
        self.key_watch.set(index, vks);
        Ok(())
    }

    /// Holds a key of the game down (`down`) or lets it go with a fake press. The mod needs an `Input` claim, else
    /// `NotPermitted`; so is a call while the panel is open, and a down while any other mod holds an active `Input`
    /// lease. A code of 256
    /// or more is `InvalidArgument`. A down for a key the mod holds, or an up for one it does not, sends nothing.
    /// An up while the real key is down only drops the hold: the real key-up lets the game see the key go up.
    pub fn send_key(&mut self, index: ModIndex, vk: u32, down: bool) -> Result<(), ManagerError> {
        self.require_input_claim(index)?;
        if vk >= InputEvent::VK_SLOTS as u32 {
            return Err(ManagerError::InvalidArgument);
        }
        let mut others = self.arbiter.holders(Capability::Input);
        others.remove(index);
        if self.panel.is_open() || (down && !others.is_empty()) {
            return Err(ManagerError::NotPermitted);
        }
        if down {
            self.fake_keys.hold(index, vk);
        } else {
            let mark = self.fake_keys.pending();
            self.fake_keys.release(index, vk);
            let (key_state, router) = (&self.key_state, &self.router);
            self.fake_keys.drop_ups_since(mark, |code| key_state.is_down(code) && !router.swallowed(code));
        }
        Ok(())
    }

    /// Moves the fake key presses the game must see into `out`, oldest first.
    pub fn take_fake_keys(&mut self, out: &mut Vec<FakeKeyOp>) {
        self.fake_keys.take_ops(out);
    }

    /// Holds a mouse button of the game down, lets it go, turns the wheel or turns the look, with a fake mouse
    /// action. The rules of `send_key` apply: the mod needs an `Input` claim, else `NotPermitted`; so is a call while
    /// the panel is open, and a button down, a wheel turn or a look turn while any other mod holds an active `Input`
    /// lease. A wheel turn of more than `FakePointer::MAX_WHEEL_STEPS` notches either way, or a look turn of more
    /// than `FakePointer::MAX_TURN_MILLIDEGREES` either way, is `InvalidArgument`; one of 0 sends nothing. Look
    /// turns of one axis add into one pending value, so a turn never meets a full queue. A
    /// down for a button the mod holds, or an up for one it does not, sends nothing. A call while the queue is full
    /// of button actions is `PointerQueueFull`, and the button state stays as it was.
    pub fn send_pointer(&mut self, index: ModIndex, op: PointerOp) -> Result<(), ManagerError> {
        self.require_input_claim(index)?;
        if let PointerOp::Wheel { steps } = op
            && steps.unsigned_abs() > FakePointer::MAX_WHEEL_STEPS.unsigned_abs()
        {
            return Err(ManagerError::InvalidArgument);
        }
        if let PointerOp::Turn { millidegrees, .. } = op
            && millidegrees.unsigned_abs() > FakePointer::MAX_TURN_MILLIDEGREES.unsigned_abs()
        {
            return Err(ManagerError::InvalidArgument);
        }
        let presses = !matches!(op, PointerOp::Button { down: false, .. });
        let mut others = self.arbiter.holders(Capability::Input);
        others.remove(index);
        if self.panel.is_open() || (presses && !others.is_empty()) {
            return Err(ManagerError::NotPermitted);
        }
        if !matches!(op, PointerOp::Turn { .. }) && !self.fake_pointer.has_room() {
            return Err(ManagerError::PointerQueueFull);
        }
        match op {
            PointerOp::Button { button, down: true } => {
                self.fake_pointer.hold(index, button);
            }
            PointerOp::Button { button, down: false } => {
                self.fake_pointer.release(index, button);
            }
            PointerOp::Wheel { steps: 0 } => {}
            PointerOp::Wheel { steps } => {
                self.fake_pointer.wheel(steps);
            }
            PointerOp::Turn { millidegrees: 0, .. } => {}
            PointerOp::Turn { axis, millidegrees } => {
                self.fake_pointer.turn(axis, millidegrees);
            }
        }
        Ok(())
    }

    /// Moves the fake mouse actions the game must see into `out`, oldest first.
    pub fn take_fake_pointer(&mut self, out: &mut Vec<PointerOp>) {
        self.fake_pointer.take_ops(out);
    }

    /// How many fake mouse actions a full queue evicted or refused since the last call.
    pub fn take_pointer_drops(&mut self) -> u64 {
        self.fake_pointer.take_dropped()
    }

    /// Sends `WorldReloadApply` to each mod with a saved `WorldReload` change since the last load.
    pub fn world_loaded(&mut self) {
        for (slot, managed) in self.mods.iter_mut().enumerate() {
            let Some(managed) = managed.as_mut().filter(|managed| managed.world_reload_pending) else {
                continue;
            };
            managed.world_reload_pending = false;
            if let Ok(slot) = u8::try_from(slot) {
                self.hub.push(ModIndex(slot), ModEvent::WorldReloadApply);
            }
        }
    }

    /// Saves each draft whose window has passed, then yields the merged writes whose window has
    /// passed. Each yielded write is taken out of the merge state.
    pub fn due_writes(&mut self, now_ms: u64) -> impl Iterator<Item = WriteRequest> + '_ {
        self.save_due_drafts(now_ms);
        self.saves.take_due(now_ms)
    }

    /// A write the config writer could not take. Its text goes back into the merge table, so it is tried again,
    /// and the save-failed toast is shown.
    pub fn write_refused(&mut self, write: WriteRequest, now_ms: u64) {
        if !self.saves.refused(write, now_ms) {
            self.toasts.push(ToastKind::SaveFailed, now_ms);
        }
        self.toasts.push(ToastKind::SaveFailed, now_ms);
    }

    pub fn toasts(&self) -> &ToastQueue {
        &self.toasts
    }

    pub fn telemetry(&self) -> &Telemetry {
        &self.telemetry
    }

    /// The telemetry counters, for a recorder on another thread.
    pub fn shared_telemetry(&self) -> Arc<Telemetry> {
        Arc::clone(&self.telemetry)
    }

    pub fn registry(&self) -> &ModRegistry {
        &self.registry
    }

    pub fn arbiter(&self) -> &LeaseArbiter {
        &self.arbiter
    }

    /// The saved config document of a mod.
    pub fn document(&self, index: ModIndex) -> Option<&HclDocument> {
        self.mods.get(usize::from(index.0))?.as_ref().map(|managed| &managed.document)
    }

    pub fn config(&self) -> &ManagerConfig {
        &self.config
    }

    // Saves each draft whose window has passed, and rebuilds the keybind table when a chord changed.
    fn save_due_drafts(&mut self, now_ms: u64) {
        let mut chords_changed = false;
        for slot in 0..self.drafts.len() {
            if self.drafts.is_due(slot, now_ms) {
                chords_changed |= self.save_slot(slot, now_ms).unwrap_or(false);
            }
        }
        if chords_changed {
            self.refresh();
        }
    }

    // Checks and applies one slot's draft and queues its write. The mod gets a `SettingsChanged` that counts its
    // changed `Live` and `WorldReload` paths; `Restart` paths are only written. Returns whether a chord changed.
    fn save_slot(&mut self, slot: usize, now_ms: u64) -> Option<bool> {
        let index = ModIndex(u8::try_from(slot).ok()?);
        let registered = self.registry.get(index)?;
        let (document, paths) = self.drafts.take(slot)?;
        if paths.is_empty() {
            return None;
        }
        let document = SchemaChecker::check(&registered.descriptor.settings, document).document;
        let fields: Vec<&SettingField> =
            paths.iter().filter_map(|path| Self::field(&registered.descriptor, path)).collect();
        let notified = fields.iter().filter(|field| field.apply != ApplyMode::Restart).count();
        let world_reload = fields.iter().any(|field| field.apply == ApplyMode::WorldReload);
        let chords = fields.iter().any(|field| matches!(field.kind, FieldKind::Chord));
        let managed = self.mods.get_mut(slot)?.as_mut()?;
        managed.document = document.clone();
        managed.world_reload_pending |= world_reload;
        let (path, text) = (managed.config_path.clone(), document.text());
        self.drafts.reset(slot, document);
        if notified > 0 {
            self.hub.push(index, ModEvent::SettingsChanged { paths: u32::try_from(notified).unwrap_or(u32::MAX) });
        }
        self.queue_mod_write(slot, path, text, now_ms);
        Some(chords)
    }

    // Edits the manager's own config document and queues its write, when the manager has a config file.
    fn write_config(&mut self, edit: impl FnOnce(&HclDocument) -> HclDocument) {
        if !self.saves.edit_config(edit) {
            self.toasts.push(ToastKind::SaveFailed, 0);
        }
    }

    fn clear_failures(&mut self, index: ModIndex) {
        if let Some(managed) = self.mods.get_mut(usize::from(index.0)).and_then(Option::as_mut) {
            managed.failures = 0;
        }
        self.registry.set_failed(index, false);
    }

    // Queues the write of a mod's config file, unless the mod is marked do-not-write or has no slot: then the change
    // stays in memory. A refusal sets the header to "Save failed", and shows the save-failed toast and a log line
    // once per toast rate window per mod.
    fn queue_mod_write(&mut self, slot: usize, path: PathBuf, text: String, now_ms: u64) {
        let rate_ms = self.config.budgets.toast_rate_ms;
        let managed = self.mods.get_mut(slot).and_then(Option::as_mut);
        if managed.as_ref().is_some_and(|managed| managed.write_back) {
            self.queue_write(path, text, now_ms);
            return;
        }
        let Some(managed) = managed else {
            return;
        };
        let limited = managed.refusal_shown_ms.is_some_and(|shown| now_ms.saturating_sub(shown) < rate_ms);
        if !limited {
            managed.refusal_shown_ms = Some(now_ms);
        }
        self.saves.mark_refused(now_ms);
        if limited {
            return;
        }
        curia::warn!("config write refused, the mod's config file could not be read and is kept as it is", {
            path: path.display().to_string(),
        });
        self.toasts.push(ToastKind::SaveFailed, now_ms);
    }

    fn queue_write(&mut self, path: PathBuf, text: String, now_ms: u64) {
        if !self.saves.queue(path, text, now_ms) {
            self.toasts.push(ToastKind::SaveFailed, now_ms);
        }
    }

    // Rebuilds everything derived from the registry, the documents and the resolutions: the
    // blocked table, the keybind table and the clash report. Then publishes the snapshot.
    fn refresh(&mut self) {
        self.arbiter.refresh_blocked(&self.registry, &self.config.resolutions);
        let mods = &self.mods;
        let documents = self.registry.iter().filter_map(|registered| {
            let managed = mods.get(usize::from(registered.index.0))?.as_ref()?;
            Some((registered.index, &registered.descriptor, &managed.document))
        });
        let chords = self.keys.collect(documents);
        self.matcher.rebuild(chords);
        self.analyzer.analyze(&self.registry, &self.config.resolutions, chords);
        self.publish();
    }

    // Takes mods that no longer hold Input or Overlay out of the cursor set, cancels the pending captures of mods
    // that no longer hold Hud or Overlay, ends the size forced by a mod that no longer holds RenderSize, then swaps
    // in a new snapshot.
    fn publish(&mut self) {
        self.track_full_block();
        self.release_fake_for_new_input_holders();
        self.captures.cancel_unleased(&self.arbiter, &mut self.hub);
        self.render_size.drop_unleased(&self.arbiter);
        for registered in self.registry.iter() {
            let index = registered.index;
            if !self.arbiter.holds(index, Capability::Input) && !self.arbiter.holds(index, Capability::Overlay) {
                self.cursor.remove(index);
            }
        }
        let snapshot = self.build_snapshot();
        self.replies.retain(&snapshot.layers);
        self.snapshot.store(Arc::new(snapshot));
    }

    fn build_snapshot(&self) -> RenderSnapshot {
        let mods = &self.mods;
        let registered_seq = |index: ModIndex| {
            mods.get(usize::from(index.0)).and_then(Option::as_ref).map_or(u64::MAX, |managed| managed.registered_seq)
        };
        RenderSnapshot {
            layers: SnapshotBuilder::layers(&self.registry, &self.arbiter, registered_seq),
            cursor_free: self.cursor.cursor_free(self.panel.is_open()),
            hide_vanilla_hud: SnapshotBuilder::hides_vanilla_hud(&self.registry, &self.arbiter),
            blocks_player_turn: SnapshotBuilder::blocks_player_turn(&self.registry, &self.arbiter)
                || self.full_block_holder.is_some(),
            blocks_gamepad: self.full_block_holder.is_some(),
            panel_open: self.panel.is_open(),
            menu_button: self.panel.menu_button_allowed() && !self.panel.is_open(),
        }
    }

    fn deliver(&mut self, ev: &InputEvent, decision: &RouteDecision, now_ms: u64) {
        if let Some((index, action, phase)) = decision.keybind {
            self.hub.push(index, ModEvent::Keybind { action, phase });
        }
        if let Some(toast) = decision.blocked {
            self.toasts.push(toast, now_ms);
        }
        let Some(raw) = Self::raw_event(ev) else {
            return;
        };
        for index in decision.raw_targets.iter() {
            self.hub.push(index, raw);
        }
        if let ModEvent::Key { vk, .. } = raw
            && !decision.consume_from_game
            && !decision.to_panel
        {
            for index in self.key_watch.watchers(vk).iter().filter(|index| !decision.raw_targets.contains(*index)) {
                self.hub.push(index, raw);
            }
        }
    }

    // Sends the release of every key and button still down to the mods that got its down, and the
    // Released phase of every keybind still pressed. With `keep`, that mod gets none and keeps them in the router;
    // the router forgets what it owed every other mod.
    fn release_held(&mut self, keep: Option<ModIndex>) {
        let mut released = ModMask::default();
        for vk in 0..InputEvent::VK_SLOTS as u32 {
            let mut targets = self.router.held_targets(vk);
            if let Some(keep) = keep {
                targets.remove(keep);
            }
            released.0 |= targets.0;
            if !targets.is_empty() {
                let (x, y) = self.panel.pointer();
                let up = match InputEvent::button_for_vk(vk) {
                    Some(button) => ModEvent::PointerButton { button, down: false, x, y },
                    None => ModEvent::Key { vk, down: false, repeat: false },
                };
                for index in targets.iter() {
                    self.hub.push(index, up);
                }
            }
            if let Some((index, action)) = self.router.held_keybind(vk).filter(|(index, _)| Some(*index) != keep) {
                released.insert(index);
                self.hub.push(index, ModEvent::Keybind { action, phase: KeybindPhase::Released });
            }
        }
        if keep.is_some() {
            for index in released.iter() {
                self.router.forget(index);
            }
        }
    }

    // The full release of a focus loss: held keys and keybinds, the router, the key state, then every fake key and
    // button.
    fn release_focus_all(&mut self) {
        self.release_held(None);
        self.router.clear();
        self.key_state.apply(&InputEvent::FocusLost);
        self.release_fake_all();
    }

    // When the full block of a holder ends, a focus loss it kept from the game runs its full release and the game
    // is owed that focus loss; without one, the holder's fake keys and buttons are let go. When a full block starts,
    // the keys and buttons the game saw go down are let go on the fake path.
    fn track_full_block(&mut self) {
        let holder = self.full_block_holder_now();
        if holder == self.full_block_holder {
            return;
        }
        let ended = std::mem::replace(&mut self.full_block_holder, holder);
        if std::mem::take(&mut self.focus_lost_deferred) {
            self.release_focus_all();
            self.focus_loss_owed = true;
        } else if let Some(ended) = ended {
            self.release_fake_mod(ended);
        }
        if holder.is_some() {
            self.release_game_held();
        }
    }

    // Queues the game's up of every real key and button it saw go down, and keeps their repeats and real ups from
    // it. A key or button a mod holds with a fake press stays down: that mod's release sends its up.
    fn release_game_held(&mut self) {
        let (fake_keys, fake_pointer) = (&mut self.fake_keys, &mut self.fake_pointer);
        self.router.take_game_held(|vk| match InputEvent::button_for_vk(vk) {
            Some(number) => PointerButton::from_number(number)
                .is_some_and(|button| fake_pointer.is_held(button) || fake_pointer.queue_game_up(button)),
            None => fake_keys.is_held(vk) || fake_keys.queue_game_up(vk),
        });
    }

    // The active exclusive Input holder, while its claim has `full_block`. The answer reads only the leases and the
    // registry, so the pointer and keyboard owners are left out of the context.
    fn full_block_holder_now(&self) -> Option<ModIndex> {
        let ctx = RouteContext {
            panel_open: self.panel.is_open(),
            toggle_chord: self.config.toggle_chord,
            pointer_owner: None,
            keyboard_owner: None,
            over_menu_button: false,
            over_game_viewport: false,
            registry: &self.registry,
            arbiter: &self.arbiter,
            book: &self.config.resolutions,
        };
        if !self.router.full_block_active(&ctx) {
            return None;
        }
        self.arbiter.exclusive_holder(Capability::Input)
    }

    // A mod that gained an active `Input` lease takes the keyboard and mouse: the fake keys and buttons of every other
    // mod are released.
    fn release_fake_for_new_input_holders(&mut self) {
        let current = self.arbiter.holders(Capability::Input);
        let gained = ModMask(current.0 & !self.input_holders.0);
        self.input_holders = current;
        if gained.is_empty() {
            return;
        }
        for slot in 0..ModMask::CAPACITY as u8 {
            if !gained.contains(ModIndex(slot)) {
                self.release_fake_mod(ModIndex(slot));
            }
        }
    }

    fn require_input_claim(&self, index: ModIndex) -> Result<(), ManagerError> {
        let registered = self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        if registered.descriptor.claims.iter().any(|claim| claim.capability == Capability::Input) {
            Ok(())
        } else {
            Err(ManagerError::NotPermitted)
        }
    }

    // Lets go of every fake key and button. Each mod that lost one gets `FakeKeysReleased`. A key whose real key
    // is down gets no up, because the real key-up reaches the game.
    fn release_fake_all(&mut self) {
        let mark = self.fake_keys.pending();
        let mut lost = self.fake_keys.release_all();
        lost.0 |= self.fake_pointer.release_all().0;
        self.finish_fake_release(mark, lost);
    }

    fn release_fake_mod(&mut self, index: ModIndex) {
        let mark = self.fake_keys.pending();
        let mut lost = self.fake_keys.release_mod(index);
        lost.0 |= self.fake_pointer.release_mod(index).0;
        self.finish_fake_release(mark, lost);
    }

    fn finish_fake_release(&mut self, mark: usize, lost: ModMask) {
        let (key_state, router) = (&self.key_state, &self.router);
        self.fake_keys.drop_ups_since(mark, |vk| key_state.is_down(vk) && !router.swallowed(vk));
        for index in lost.iter() {
            self.hub.push(index, ModEvent::FakeKeysReleased);
        }
    }

    fn forget_reply(&mut self, index: ModIndex) {
        self.replies.forget(index);
    }

    fn raw_event(ev: &InputEvent) -> Option<ModEvent> {
        match *ev {
            InputEvent::Key { vk, down, repeat, .. } => Some(ModEvent::Key { vk, down, repeat }),
            InputEvent::PointerButton { button, down, x, y } => Some(ModEvent::PointerButton { button, down, x, y }),
            InputEvent::PointerDelta { dx, dy } => Some(ModEvent::PointerDelta { dx, dy }),
            InputEvent::Wheel { delta } => Some(ModEvent::Wheel { delta }),
            InputEvent::PointerMove { .. } | InputEvent::Char(_) | InputEvent::FocusLost => None,
        }
    }

    fn field<'a>(descriptor: &'a Descriptor, path: &str) -> Option<&'a SettingField> {
        descriptor.settings.iter().flat_map(|section| &section.fields).find(|field| field.path == path)
    }

    // One HCL expression evaluated without variables.
    fn parse_value(text: &str) -> Option<hcl::Value> {
        let expression = text.parse::<hcl_edit::expr::Expression>().ok()?;
        hcl::Expression::from(expression).evaluate(&Context::new()).ok()
    }

    fn setting_error(path: &str) -> ManagerError {
        ManagerError::Setting { path: path.to_string() }
    }
}
