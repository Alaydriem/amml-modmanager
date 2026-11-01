use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{FrameState, StopLatch};
use crate::traits::GameCalls;

/// Gives the mouse to the layers and back. The `MinecraftGame*` comes from the game's own `grabMouse`. Each tick
/// releases or grabs the mouse when `cursor_free` changed, but only while the player is in play, since the game
/// manages the mouse itself in menus. Game thread, except `keep_game`.
pub struct MouseGrab {
    game: AtomicUsize,
    // the cursor_free value the last release or grab acted on
    applied: AtomicBool,
}

impl MouseGrab {
    pub const fn new() -> Self {
        Self { game: AtomicUsize::new(0), applied: AtomicBool::new(false) }
    }

    /// The `this` of the game's `MinecraftGame::grabMouse`.
    pub fn keep_game(&self, game: usize) {
        if game != 0 {
            self.game.store(game, Ordering::Relaxed);
        }
    }

    /// The game's own grab is skipped while the layers keep the cursor free.
    pub fn blocks_grab(frame: &FrameState) -> bool {
        frame.cursor_free()
    }

    /// The player is in a world with no screen on top: a local player, an active world and no menu.
    pub fn in_play(calls: &dyn GameCalls, client: usize) -> bool {
        client != 0 && calls.local_player(client) != 0 && calls.world_active(client) && !calls.showing_menu(client)
    }

    /// Once per tick, after the host tick.
    pub fn apply(&self, calls: &dyn GameCalls, latch: &StopLatch, frame: &FrameState, client: usize) {
        if latch.raised() {
            return;
        }
        let free = frame.cursor_free();
        let game = self.game.load(Ordering::Relaxed);
        if free == self.applied.load(Ordering::Relaxed) || game == 0 || !Self::in_play(calls, client) {
            return;
        }
        self.applied.store(free, Ordering::Relaxed);
        if free {
            calls.release_mouse(game);
        } else {
            calls.grab_mouse(game);
        }
    }

    /// At disable: when the manager had freed the cursor, the window has focus and the player is in play, the
    /// game gets its mouse back once.
    pub fn restore(&self, calls: &dyn GameCalls, client: usize, foreground: bool) {
        let was_free = self.applied.swap(false, Ordering::Relaxed);
        let game = self.game.load(Ordering::Relaxed);
        if was_free && foreground && game != 0 && Self::in_play(calls, client) {
            calls.grab_mouse(game);
        }
    }
}

impl Default for MouseGrab {
    fn default() -> Self {
        Self::new()
    }
}
