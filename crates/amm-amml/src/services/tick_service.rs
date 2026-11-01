use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};

use crate::traits::GameCalls;

/// The game-thread side of the manager: the host tick, the world events, the `ClientInstance*` of the last tick,
/// and whether the game has ticked, which binds the overlay.
pub struct TickService {
    client: AtomicUsize,
    ticked: AtomicBool,
    last_tick_ms: AtomicU64,
}

impl TickService {
    pub const fn new() -> Self {
        Self { client: AtomicUsize::new(0), ticked: AtomicBool::new(false), last_tick_ms: AtomicU64::new(0) }
    }

    /// One client tick, after the game's own update of `client`.
    pub fn tick(&self, calls: &dyn GameCalls, client: usize) {
        self.client.store(client, Ordering::Relaxed);
        let now = calls.now_ms();
        calls.host_tick(now);
        self.last_tick_ms.store(now, Ordering::Relaxed);
        self.ticked.store(true, Ordering::SeqCst);
    }

    /// A world finished loading (`true`) or is being left (`false`), before the game's own handling. Leaving
    /// forgets the client until the next tick.
    pub fn world(&self, calls: &dyn GameCalls, loaded: bool) {
        calls.host_world(loaded);
        if !loaded {
            self.client.store(0, Ordering::Relaxed);
        }
    }

    /// The game has run at least one tick.
    pub fn game_ready(&self) -> bool {
        self.ticked.load(Ordering::SeqCst)
    }

    /// The `ClientInstance*` of the last tick; 0 before the first and after leaving a world.
    pub fn client(&self) -> usize {
        self.client.load(Ordering::Relaxed)
    }

    /// A tick ran at most `window_ms` before `now_ms`.
    pub fn ticked_within(&self, now_ms: u64, window_ms: u64) -> bool {
        self.game_ready() && now_ms.saturating_sub(self.last_tick_ms.load(Ordering::Relaxed)) <= window_ms
    }
}

impl Default for TickService {
    fn default() -> Self {
        Self::new()
    }
}
