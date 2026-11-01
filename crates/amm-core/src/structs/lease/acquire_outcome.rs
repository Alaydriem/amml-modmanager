use crate::structs::registry::ModIndex;

/// The result of asking for a capability lease.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcquireOutcome {
    Granted,
    Denied { holder: ModIndex },
    Preempted,
}
