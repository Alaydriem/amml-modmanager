use amml_api::{ApiError, Input, InputBlocks, InputLease, LeaseMode, LeaseRequest, LeaseState};

/// The input lease the manager holds while its panel is open: exclusive, blocking all real input and preempting
/// every other lease, so a mod that holds keys through its own lease is suspended, its keys get their ups once,
/// and it gets `InjectedInputReleased` with `SUSPENDED`. Dropping it ends the lease, and the suspended leases are
/// active again with no key pressed.
#[must_use = "dropping the panel lease ends it"]
pub struct PanelLease {
    lease: InputLease,
}

impl PanelLease {
    pub const REQUEST: LeaseRequest =
        LeaseRequest { mode: LeaseMode::Exclusive, blocks: InputBlocks::FULL, passthrough: &[], preempt: true };

    /// Takes the lease. `Refused` while another mod holds a preempt lease; `NotAvailable` without an input table.
    pub fn open(input: &Input) -> Result<PanelLease, ApiError> {
        input.acquire(Self::REQUEST).map(|lease| Self { lease })
    }

    pub fn state(&self) -> LeaseState {
        self.lease.state()
    }
}
