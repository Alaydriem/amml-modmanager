use std::sync::atomic::{AtomicI32, AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use amm_core::structs::input::FakeKeyOp;

use crate::traits::GameCalls;

/// Feeds the fake key presses mods queued with `send_key` to the game's own keyboard functions, past the
/// manager's key detour, so a fake key is never routed back to the manager.
///
/// The HIDController `this` and the `InputOrigin` come from the last real key press (`note`, called from the
/// `key_down` detour). `flush` runs from `ManagerRuntime::tick` after the host tick: `ClientInstance::update` and
/// the key hooks run on the same thread, so the game's keyboard functions are called from the thread that feeds
/// them. Before the first real key press there is no controller, and `flush` leaves the presses queued in the host.
pub struct FakeKeySender {
    controller: AtomicUsize,
    origin: AtomicI32,
    // reused between ticks; one tick never holds more than the host queue, which the host bounds
    ops: Mutex<Vec<FakeKeyOp>>,
}

impl FakeKeySender {
    /// The `InputOrigin` of a real key press, used until a real key press has set one.
    pub const DEFAULT_ORIGIN: i32 = 1;

    pub const fn new() -> Self {
        Self {
            controller: AtomicUsize::new(0),
            origin: AtomicI32::new(Self::DEFAULT_ORIGIN),
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

    /// Takes every queued press from the host and sends it, oldest first.
    pub fn flush(&self, calls: &dyn GameCalls) {
        let controller = self.controller.load(Ordering::Relaxed);
        if controller == 0 {
            return;
        }
        let origin = self.origin.load(Ordering::Relaxed);
        let mut ops = self.ops.lock().unwrap_or_else(PoisonError::into_inner);
        ops.clear();
        calls.take_fake_keys(&mut ops);
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
