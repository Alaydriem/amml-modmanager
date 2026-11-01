use std::cell::Cell;

/// Marks the current thread as inside the host lock for as long as it lives, and clears the mark on drop, panic
/// included. A table call made from inside a callback the host runs under its lock finds the mark and is refused
/// instead of waiting on the lock its own thread holds.
pub struct ReentryGuard;

thread_local! {
    static INSIDE: Cell<bool> = const { Cell::new(false) };
}

impl ReentryGuard {
    /// The guard, or `None` when this thread is already inside the host lock.
    pub fn enter() -> Option<ReentryGuard> {
        if INSIDE.with(Cell::get) {
            return None;
        }
        INSIDE.with(|inside| inside.set(true));
        Some(Self)
    }
}

impl Drop for ReentryGuard {
    fn drop(&mut self) {
        INSIDE.with(|inside| inside.set(false));
    }
}
