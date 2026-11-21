/// Where a `ManagerLink` is with the manager.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    /// The link listens and waits for the manager's tables. Calls give `LinkError::NotConnected`, or the kept
    /// `LinkError::NoManager` once every mod has loaded without a manager.
    Waiting,
    /// The mod is registered with the manager and its calls reach the tables.
    Connected,
    /// The manager is being disabled or is gone. Calls give `LinkError::Gone` and reach no table.
    Gone,
}
