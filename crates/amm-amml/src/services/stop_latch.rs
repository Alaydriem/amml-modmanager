use std::sync::atomic::{AtomicBool, Ordering};

/// Raised by disable before the cursor and the HUD are put back, so no frame or tick still running can publish or
/// apply the manager's cursor or HUD state again afterwards; lowered by enable.
pub struct StopLatch {
    raised: AtomicBool,
}

impl StopLatch {
    pub const fn new() -> Self {
        Self { raised: AtomicBool::new(false) }
    }

    pub fn raise(&self) {
        self.raised.store(true, Ordering::SeqCst);
    }

    pub fn lower(&self) {
        self.raised.store(false, Ordering::SeqCst);
    }

    pub fn raised(&self) -> bool {
        self.raised.load(Ordering::SeqCst)
    }
}

impl Default for StopLatch {
    fn default() -> Self {
        Self::new()
    }
}
