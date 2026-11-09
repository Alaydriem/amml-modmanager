use std::sync::OnceLock;

use amml_sdk::{ModRuntime, SdkError};

use super::{ManagerLifecycle, ManagerState, MouseGrab, MouseInput};
use crate::paths::ManagerPaths;
use crate::traits::GameCalls;
use crate::types::DisableOutcome;

static RUNTIME: OnceLock<ManagerRuntime> = OnceLock::new();

/// The one runtime of this DLL, made at load. The detours have no context pointer, so they reach it through
/// `get`.
pub struct ManagerRuntime {
    state: &'static ManagerState,
    calls: &'static dyn GameCalls,
    paths: ManagerPaths,
    lifecycle: ManagerLifecycle,
}

impl ManagerRuntime {
    pub fn new(state: &'static ManagerState, calls: &'static dyn GameCalls, paths: ManagerPaths) -> Self {
        Self { state, calls, paths, lifecycle: ManagerLifecycle::new(state) }
    }

    /// Keeps `runtime` for the detours. The first call wins; later calls return the first.
    pub fn install(runtime: Self) -> &'static Self {
        RUNTIME.get_or_init(|| runtime)
    }

    pub fn get() -> Option<&'static Self> {
        RUNTIME.get()
    }

    pub fn enable(&self) -> Result<(), SdkError> {
        self.lifecycle.enable(self.calls, &self.paths)
    }

    pub fn disable(&self) -> DisableOutcome {
        self.lifecycle.disable(self.calls)
    }

    /// `ClientInstance::update` of `client` ran: the host tick, the fake key presses and mouse actions mods queued,
    /// the focus loss a full block kept from the game, the screen on top for the next host frame, then the mouse grab
    /// and the HUD state the last host frame asked for.
    pub fn tick(&self, client: usize) {
        let state = self.state;
        state.tick().tick(self.calls, client);
        state.fake_keys().flush(self.calls);
        state.fake_pointer().flush(self.calls, client);
        self.replay_focus_loss();
        state.menu().observe(self.calls, client);
        state.grab().apply(self.calls, state.latch(), state.frame(), client);
        state.hud().apply(self.calls, state.latch(), state.frame(), client);
    }

    // A full block ended that had kept a focus loss from the game: while the window is still in the background, the
    // game's window procedure sees that focus loss, so the game lets go of its cursor clip.
    fn replay_focus_loss(&self) {
        if self.calls.take_focus_loss_owed() && !self.calls.game_window_foreground() {
            self.calls.replay_focus_loss();
        }
    }

    /// A key from the game's keyboard feed; true when the game must not see it.
    pub fn key(&self, key: i32, down: bool) -> bool {
        self.state.keys().route(self.calls, key, down)
    }

    /// One event of the game's mouse feed on `device`; true when the game must not see it. `x` and `y` may be
    /// moved. The device and the point the game gets are where fake mouse actions go.
    pub fn mouse(&self, device: usize, action: u8, x: &mut i16, y: &mut i16, dx: i16, dy: i16) -> bool {
        let cancel = MouseInput::feed(self.calls, action, x, y, dx, dy);
        self.state.fake_pointer().note(device, *x, *y);
        cancel
    }

    /// The game's own `MinecraftGame::grabMouse` on `game`; true when it is skipped.
    pub fn grab_called(&self, game: usize) -> bool {
        self.state.grab().keep_game(game);
        MouseGrab::blocks_grab(self.state.frame())
    }

    /// The player's own look turn is dropped.
    pub fn blocks_turn(&self) -> bool {
        self.calls.blocks_player_turn()
    }

    /// The game's gamepad poll is dropped.
    pub fn blocks_gamepad(&self) -> bool {
        self.calls.blocks_gamepad()
    }

    /// A world finished loading.
    pub fn world_loaded(&self) {
        self.state.tick().world(self.calls, true);
    }

    /// The world is being left.
    pub fn world_left(&self) {
        self.state.tick().world(self.calls, false);
    }

    pub fn state(&self) -> &'static ManagerState {
        self.state
    }

    /// A panic in a detour switched the manager off: every detour only calls the game's original, and the overlay
    /// draws nothing, until the game restarts.
    pub fn is_off(&self) -> bool {
        ModRuntime::is_disabled()
    }
}
