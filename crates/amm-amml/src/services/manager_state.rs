use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use amml_api::amml_abi::{KeyInput, MouseGrab, MouseInput, WindowChar, WindowCursor, WindowFocus, WindowPointer};
use amml_api::{
    Area, Cancel, Ctx, Event, EventCtx, Input, KeyInputExt, Logger, Window, WindowCharExt, WindowCursorExt,
    WindowFocusExt, WindowPointerExt,
};

use super::{
    ClientFlags, KeyRouter, ManagerClock, ManagerInputSink, ManagerOverlayHost, PanelLayoutLog, PanelLease,
    RankingSync, TickService, WindowRouter,
};

/// What the manager's game-thread and window-thread listeners share while it is enabled: the key router, the tick
/// with the client flags, the panel lease, the ranking, the window router and the panel layout line. Each piece has
/// its own lock; no lock is held while another piece's lock is taken, except the panel lease inside a key or a
/// tick. The listeners copy `Input`, `Logger` and `Window`; they are `Copy`.
pub struct ManagerState {
    input: Input,
    log: Logger,
    window: Window,
    sink: ManagerInputSink,
    keys: Mutex<KeyRouter>,
    tick: Mutex<TickService>,
    panel: Mutex<Option<PanelLease>>,
    ranking: Mutex<RankingSync>,
    router: Mutex<WindowRouter>,
    layout: PanelLayoutLog,
    // a panel lease the loader refused was logged; cleared when a lease is taken
    panel_refusal_logged: AtomicBool,
    // the panel state the last panel line was written for
    panel_seen: AtomicBool,
}

impl ManagerState {
    /// `menu` is the screen code the tick writes for the host frames; `host` gives the cursor shape of the last
    /// frame to the window router.
    pub fn new(
        ctx: &Ctx,
        sink: ManagerInputSink,
        clock: ManagerClock,
        menu: Arc<AtomicU8>,
        host: &'static ManagerOverlayHost,
    ) -> Self {
        let log = ctx.log();
        let window = ctx.window();
        let flags = ClientFlags::new(log, ctx.supports(Area::CLIENT_HUD), ctx.supports(Area::CLIENT_MOUSE));
        Self {
            input: ctx.input(),
            log,
            window,
            sink,
            keys: Mutex::new(KeyRouter::new(sink, clock)),
            tick: Mutex::new(TickService::new(sink, clock, flags, menu)),
            panel: Mutex::new(None),
            ranking: Mutex::new(RankingSync::new()),
            router: Mutex::new(WindowRouter::new(sink, clock, window, log, host)),
            layout: PanelLayoutLog::new(),
            panel_refusal_logged: AtomicBool::new(false),
            panel_seen: AtomicBool::new(false),
        }
    }

    /// `KeyInput`, in `HookOrder::FIRST`: the key goes to the key router, and the game does not see it when the
    /// host keeps it or while the panel is open. An injected key (a mod's own `KeyHold`) is not routed.
    pub fn on_key(&self, event: &mut KeyInput) {
        if event.is_injected() {
            return;
        }
        let modifiers = event.modifiers().with_f3(self.input.modifiers().f3());
        let route = Self::lock(&self.keys).on_key(
            event.key_code(),
            event.down != 0,
            event.repeat != 0,
            modifiers,
            event.is_cancelled(),
        );
        let open = self.sink.panel_open();
        if route.consume || open {
            event.cancel();
        }
        self.note_panel(open, true);
        self.sync_panel(open);
    }

    /// `MouseInput`, in `HookOrder::FIRST`: the game gets no mouse feed while the panel is open.
    pub fn on_mouse(&self, event: &mut MouseInput) {
        if !event.is_injected() && self.sink.panel_open() {
            event.cancel();
        }
    }

    /// `MouseGrab`, in `HookOrder::FIRST`: the game does not grab the mouse while it must stay free.
    pub fn on_grab(&self, event: &mut MouseGrab) {
        let free = self.sink.wants(false).mouse_free;
        if Self::lock(&self.tick).flags().blocks_grab(free) {
            event.cancel();
        }
    }

    /// `WindowPointer`, in `HookOrder::FIRST`: the game does not get a pointer message the panel or a layer took.
    pub fn on_pointer(&self, event: &mut WindowPointer) {
        let taken = Self::lock(&self.router).pointer(
            event.kind(),
            event.buffer(),
            event.button(),
            event.down(),
            event.notches(),
        );
        if taken {
            event.cancel();
        }
    }

    /// `WindowChar`, in `HookOrder::FIRST`: the game does not get a character the panel or a layer took.
    pub fn on_char(&self, event: &mut WindowChar) {
        let Some(ch) = event.ch() else {
            return;
        };
        if Self::lock(&self.router).char(ch) {
            event.cancel();
        }
    }

    /// `WindowFocus`, in `HookOrder::FIRST`: a loss is held back while the panel or a full block holds it; a replayed
    /// loss passes, and a gain forgets the held losses.
    pub fn on_focus(&self, event: &mut WindowFocus) {
        let mut router = Self::lock(&self.router);
        if event.gained() {
            router.focus_gained();
        } else if !event.replayed() && router.focus_lost() {
            event.cancel();
        }
    }

    /// `WindowCursor`, in `HookOrder::WRITE`: the manager's shape while the cursor belongs to it; otherwise the event
    /// is left as it came.
    pub fn on_cursor(&self, event: &mut WindowCursor) {
        if let Some(shape) = Self::lock(&self.router).cursor() {
            event.set_shape(shape);
        }
    }

    /// `ClientTick`: the tick, then the panel lease, the lease ranking, the panel lines and a held focus loss follow
    /// the panel.
    pub fn on_tick(&self, at: &EventCtx) {
        Self::lock(&self.tick).on_tick(at);
        let open = self.sink.panel_open();
        self.note_panel(open, false);
        self.sync_panel(open);
        self.sync_ranking();
        self.log_layout(open);
        Self::lock(&self.router).release_focus();
    }

    /// Pushes the lease ranking and the coexist pairs of the operator's resolutions when they differ from the last
    /// push. `enable` calls it first, so the manager owns the lease policy.
    pub fn sync_ranking(&self) {
        let mut sync = Self::lock(&self.ranking);
        if let Some(ranking) = self.sink.ranking_if_changed(sync.last()) {
            let count = ranking.len().to_string();
            match sync.push(&self.input, &ranking) {
                Ok(()) => self.log.info("lease ranking set", &[("mods", &count)]),
                Err(error) => {
                    self.log.warn("lease ranking refused", &[("mods", &count), ("error", &error.to_string())]);
                }
            }
        }
        if let Some(pairs) = self.sink.coexist_if_changed(sync.last_coexist()) {
            let count = pairs.len().to_string();
            match sync.push_coexist(&self.input, &pairs) {
                Ok(()) => self.log.info("lease coexist pairs set", &[("pairs", &count)]),
                Err(error) => {
                    self.log.warn("lease coexist pairs refused", &[("pairs", &count), ("error", &error.to_string())]);
                }
            }
        }
    }

    /// Replays a focus loss the window router holds back, for a disable while the game runs.
    pub fn replay_held_focus(&self) {
        Self::lock(&self.router).replay_held();
    }

    /// Takes the panel lease out, for a disable to drop it outside the lock.
    pub fn take_panel_lease(&self) -> Option<PanelLease> {
        Self::lock(&self.panel).take()
    }

    /// Whether the manager holds its panel lease now.
    pub fn panel_lease_held(&self) -> bool {
        Self::lock(&self.panel).is_some()
    }

    /// The screen code of the last tick, for the host frame.
    pub fn menu_screen(&self) -> u8 {
        Self::lock(&self.tick).menu_screen()
    }

    // One line per panel change: `panel opened by=keys` or `panel closed by=keys` for a change a key made; at a tick,
    // `panel opened by=ui` (the menu button) or `panel action action=Close` (the panel's own close).
    fn note_panel(&self, open: bool, by_key: bool) {
        if self.panel_seen.swap(open, Ordering::Relaxed) == open {
            return;
        }
        match (open, by_key) {
            (true, true) => self.log.info("panel opened", &[("by", "keys")]),
            (false, true) => self.log.info("panel closed", &[("by", "keys")]),
            (true, false) => self.log.info("panel opened", &[("by", "ui")]),
            (false, false) => self.log.info("panel action", &[("action", "Close")]),
        }
    }

    // The layout line once per open, after the panel's first frame placed its controls.
    fn log_layout(&self, open: bool) {
        if !open {
            self.layout.on_close();
            return;
        }
        if self.layout.logged() {
            return;
        }
        if let Some(layout) = self.sink.panel_layout() {
            self.layout.on_open(&self.log, &layout, &self.window);
        }
    }

    // Opens the panel lease while the panel is open and ends it when the panel closes.
    fn sync_panel(&self, open: bool) {
        let mut panel = Self::lock(&self.panel);
        if panel.is_some() == open {
            return;
        }
        if !open {
            *panel = None;
            return;
        }
        match PanelLease::open(&self.input) {
            Ok(lease) => {
                *panel = Some(lease);
                self.panel_refusal_logged.store(false, Ordering::Relaxed);
            }
            Err(error) => {
                if !self.panel_refusal_logged.swap(true, Ordering::Relaxed) {
                    self.log.warn("panel input lease refused", &[("error", &error.to_string())]);
                }
            }
        }
    }

    fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
        mutex.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
