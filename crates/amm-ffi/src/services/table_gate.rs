use std::sync::atomic::{AtomicBool, Ordering};

/// Whether the manager's registry tables still answer. It closes once, when the manager is disabled, and stays closed
/// for the rest of the host: the tables are statics of a DLL the loader never unmaps, so a consumer that calls one
/// late gets `Disabled`, never a host that is gone.
pub struct TableGate {
    closed: AtomicBool,
}

impl TableGate {
    pub fn new() -> Self {
        Self { closed: AtomicBool::new(false) }
    }

    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }
}

impl Default for TableGate {
    fn default() -> Self {
        Self::new()
    }
}
