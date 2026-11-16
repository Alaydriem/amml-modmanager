use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use amm_core::structs::input::FakeKeyOp;

use crate::traits::GameCalls;

/// Feeds the fake key presses mods queued with `send_key` to the game's own keyboard functions, past the
/// manager's key detour, so a fake key is never routed back to the manager.
///
/// The HIDController `this` comes from the game's own update of the controller (`note_controller`, called from the
/// `controller_update` detour), so no real key press is needed, or from the last real key press (`note`, called from
/// the `key_down` detour), which also gives the `InputOrigin`. `flush` runs from `ManagerRuntime::tick` after the
/// host tick: `ClientInstance::update` and the key hooks run on the same thread, so the game's keyboard functions are
/// called from the thread that feeds them. Each flush tells the host whether a controller is known; while none is,
/// the host refuses a fake key down with `NotReady` and `flush` takes nothing.
pub struct FakeKeySender {
    controller: AtomicUsize,
    origin: AtomicI32,
    // the game's update has noted the controller at least once; read by `flush` for the log lines
    update_seen: AtomicBool,
    update_logged: AtomicBool,
    found_logged: AtomicBool,
    // reused between ticks; one tick never holds more than the host queue, which the host bounds
    ops: Mutex<Vec<FakeKeyOp>>,
}

impl FakeKeySender {
    /// The `InputOrigin` of a real key press, used until a real key press has set one.
    pub const DEFAULT_ORIGIN: i32 = 1;
    /// The info line, written once, when the game's own update of the keyboard controller first reached the manager.
    /// It shows that the `controller_update` hook runs, so fake keys need no real key press.
    pub const UPDATE_LINE: &'static str = "the keyboard controller is learned from the game update";

    pub const fn new() -> Self {
        Self {
            controller: AtomicUsize::new(0),
            origin: AtomicI32::new(Self::DEFAULT_ORIGIN),
            update_seen: AtomicBool::new(false),
            update_logged: AtomicBool::new(false),
            found_logged: AtomicBool::new(false),
            ops: Mutex::new(Vec::new()),
        }
    }

    /// A real key went down on `controller` with `origin`.
    pub fn note(&self, controller: usize, origin: i32) {
        if controller != 0 {
            self.controller.store(controller, Ordering::Relaxed);
            self.origin.store(origin, Ordering::Relaxed);
        }
    }

    /// The game updated its keyboard `controller`. The origin stays the one of the last real key press. It runs
    /// every frame from the detour: two `Relaxed` loads, and a store only when a value changes.
    pub fn note_controller(&self, controller: usize) {
        if controller == 0 {
            return;
        }
        if self.controller.load(Ordering::Relaxed) != controller {
            self.controller.store(controller, Ordering::Relaxed);
        }
        if !self.update_seen.load(Ordering::Relaxed) {
            self.update_seen.store(true, Ordering::Relaxed);
        }
    }

    /// Tells the host whether a controller is known; with one, takes every queued press from the host and sends it,
    /// oldest first.
    pub fn flush(&self, calls: &dyn GameCalls) {
        let controller = self.controller.load(Ordering::Relaxed);
        let mut ops = self.ops.lock().unwrap_or_else(PoisonError::into_inner);
        ops.clear();
        calls.take_fake_keys(controller != 0, &mut ops);
        if controller == 0 {
            return;
        }
        let update_seen = self.update_seen.load(Ordering::Relaxed);
        if !self.found_logged.swap(true, Ordering::Relaxed) {
            curia::info!("fake keys reach the game, the keyboard controller is known", { from_game_update: update_seen });
        }
        if update_seen && !self.update_logged.swap(true, Ordering::Relaxed) {
            curia::info!("the keyboard controller is learned from the game update", { controller: controller });
        }
        let origin = self.origin.load(Ordering::Relaxed);
        for op in ops.iter() {
            calls.game_key(controller, op.vk, op.down, origin);
        }
    }
}

impl Default for FakeKeySender {
    fn default() -> Self {
        Self::new()
    }
}
