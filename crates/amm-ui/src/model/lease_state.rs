use amm_core::structs::registry::ModIndex;

/// The run-time state of one capability for one mod, as the Capabilities tab shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    NotClaimed,
    Idle,
    Held,
    Suspended { by: ModIndex },
    Blocked { by: ModIndex },
}
