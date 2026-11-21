use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use arc_swap::ArcSwap;
use hcl::eval::{Context, Evaluate};

use crate::config::{ExclusiveResolution, ManagerConfig, PairKey, SuspensionResolution};
use crate::errors::ManagerError;
use crate::services::clash::ClashAnalyzer;
use crate::services::descriptor::DescriptorChecker;
use crate::services::event::EventHub;
use crate::services::icon::IconBook;
use crate::services::input::{ChordMatcher, CursorSet, GameViewportBook, InputRouter};
use crate::services::lease::LeaseArbiter;
use crate::services::menu::MenuEntries;
use crate::services::registry::ModRegistry;
use crate::services::render::RenderSizeBook;
use crate::services::settings::{ChordText, DraftBook, EffectiveKeys, ManagerConfigFile, SchemaChecker};
use crate::services::snapshot::SnapshotBuilder;
use crate::services::telemetry::Telemetry;
use crate::services::toast::ToastQueue;
use crate::structs::clash::ClashReport;
use crate::structs::descriptor::{ApplyMode, Capability, Chord, Descriptor, FieldKind, SettingField};
use crate::structs::event::{KeybindPhase, ModEvent};
use crate::structs::frame::{FrameReplyData, LayerKind, MenuScreen};
use crate::structs::icon::{IconEntry, ModIcon};
use crate::structs::input::{GameViewport, InputEvent, RouteContext, RouteDecision};
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
        let registry = ModRegistry::new(slots);
        let mut arbiter = LeaseArbiter::new();
        arbiter.refresh_blocked(&registry, &config.resolutions);
        Self {
            checker,
            granter: TextureGranter::new(budgets),
            registry,
            arbiter,
            hub,
            toasts,
            analyzer: ClashAnalyzer::new(),
            keys: EffectiveKeys::new(),
            matcher: ChordMatcher::new(EffectiveKeys::CAPACITY),
            router: InputRouter::new(slots),
            cursor: CursorSet::new(),
            viewports: GameViewportBook::new(),
            render_size: RenderSizeBook::new(),
            panel: PanelState::new(config.menu_button),
            menu: MenuEntries::new(),
            icons: IconBook::new(slots),
            locale_readers: ModMask::default(),
            replies: LayerReplies::new(slots),
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
        self.arbiter.release_all(index, &mut self.hub);
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

    /// Asks for a lease. A disabled mod holds no leases, so its request is `NotPermitted`, as is a capability the
    /// manager does not lease (`Input`, whose lease is the loader's). A denial pushes the denial toast, within its
    /// rate limit.
    pub fn acquire(&mut self, index: ModIndex, cap: Capability, now_ms: u64) -> Result<AcquireOutcome, ManagerError> {
        let registered = self.registry.get(index).ok_or(ManagerError::UnknownMod)?;
        if !registered.enabled || !Capability::MANAGER.contains(&cap) {
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
    /// `Overlay` lease; giving it back is always allowed.
    pub fn set_cursor_mode(&mut self, index: ModIndex, on: bool) -> Result<(), ManagerError> {
        if self.registry.get(index).is_none() {
            return Err(ManagerError::UnknownMod);
        }
        if on && !self.arbiter.holds(index, Capability::Overlay) {
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

    /// Places one input event, pushes its keybind event to the mod and its blocked toast to the toast queue, and
    /// returns the decision for the caller (cancel the game event, toggle or feed the panel, the layers it reaches).
    /// On focus loss the `Released` phase of every keybind still pressed goes to its mod, so no bind stays down.
    pub fn route(&mut self, ev: &InputEvent, now_ms: u64) -> RouteDecision {
        let started = self.config.telemetry.then(Instant::now);
        let decision = self.route_event(ev, now_ms);
        self.telemetry.record_event_drops(self.hub.dropped());
        if let Some(started) = started {
            self.telemetry.record_route(u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX));
        }
        decision
    }

    fn route_event(&mut self, ev: &InputEvent, now_ms: u64) -> RouteDecision {
        if *ev == InputEvent::FocusLost {
            self.release_held();
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
        let decision = self.router.route(ev, &ctx, &self.matcher);
        self.deliver(&decision, now_ms);
        decision
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

    /// Opens or closes the panel.
    pub fn set_panel_open(&mut self, open: bool) {
        if self.panel.set_open(open) {
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

    /// The chord keybind `action` of the mod in `index` uses now: the chord stored at its `config_path`, or its
    /// default when that value is missing or not a chord. `None` for a slot with no mod or an action past its
    /// descriptor's list.
    pub fn keybind_chord(&self, index: ModIndex, action: u16) -> Option<Chord> {
        let registered = self.registry.get(index)?;
        let keybind = registered.descriptor.keybinds.get(usize::from(action))?;
        let stored = self.document(index).and_then(|document| document.get(&keybind.config_path));
        let chord = stored.as_ref().and_then(hcl::Value::as_str).and_then(ChordText::parse);
        Some(chord.unwrap_or(keybind.default_chord))
    }

    /// Whether keybind `action` of the mod in `index` is held down: its `Pressed` event went out and its `Released`
    /// has not.
    pub fn keybind_down(&self, index: ModIndex, action: u16) -> bool {
        self.registry.get(index).is_some() && self.router.bind_held(index, action)
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

    // Takes mods that no longer hold Overlay out of the cursor set, ends the size forced by a mod that no longer holds
    // RenderSize, then swaps in a new snapshot.
    fn publish(&mut self) {
        self.render_size.drop_unleased(&self.arbiter);
        for registered in self.registry.iter() {
            let index = registered.index;
            if !self.arbiter.holds(index, Capability::Overlay) {
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
            panel_open: self.panel.is_open(),
            menu_button: self.panel.menu_button_allowed() && !self.panel.is_open(),
        }
    }

    fn deliver(&mut self, decision: &RouteDecision, now_ms: u64) {
        if let Some((index, action, phase)) = decision.keybind {
            self.hub.push(index, ModEvent::Keybind { action, phase });
        }
        if let Some(toast) = decision.blocked {
            self.toasts.push(toast, now_ms);
        }
    }

    // Sends the Released phase of every keybind still pressed to its mod.
    fn release_held(&mut self) {
        for vk in 0..InputEvent::VK_SLOTS as u32 {
            if let Some((index, action)) = self.router.held_keybind(vk) {
                self.hub.push(index, ModEvent::Keybind { action, phase: KeybindPhase::Released });
            }
        }
    }

    fn forget_reply(&mut self, index: ModIndex) {
        self.replies.forget(index);
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
