use std::sync::atomic::{AtomicBool, Ordering};

use super::{FrameState, StopLatch};
use crate::traits::GameCalls;

/// Hides the vanilla HUD while a host frame asks for it, and puts the player's own setting back after. Game
/// thread.
pub struct HudControl {
    // the manager hides the HUD now
    hidden: AtomicBool,
    // the player's hide-HUD value from before the manager hid it
    player_hide: AtomicBool,
}

impl HudControl {
    pub const fn new() -> Self {
        Self { hidden: AtomicBool::new(false), player_hide: AtomicBool::new(false) }
    }

    /// Once per tick, after the host tick.
    pub fn apply(&self, calls: &dyn GameCalls, latch: &StopLatch, frame: &FrameState, client: usize) {
        if latch.raised() || client == 0 {
            return;
        }
        let wanted = frame.hide_hud();
        if wanted == self.hidden.load(Ordering::Relaxed) {
            return;
        }
        let Some(options) = calls.options(client) else {
            return;
        };
        if wanted {
            self.player_hide.store(calls.hide_hud(options), Ordering::Relaxed);
            calls.set_hide_gui(options, true);
        } else {
            calls.set_hide_gui(options, self.player_hide.load(Ordering::Relaxed));
        }
        self.hidden.store(wanted, Ordering::Relaxed);
    }

    /// At disable: puts the player's setting back if the manager hid the HUD.
    pub fn restore(&self, calls: &dyn GameCalls, client: usize) {
        if client == 0 || !self.hidden.load(Ordering::Relaxed) {
            return;
        }
        let Some(options) = calls.options(client) else {
            return;
        };
        calls.set_hide_gui(options, self.player_hide.load(Ordering::Relaxed));
        self.hidden.store(false, Ordering::Relaxed);
    }
}

impl Default for HudControl {
    fn default() -> Self {
        Self::new()
    }
}
