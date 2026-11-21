/// The result code of a table function.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmStatus {
    Ok = 0,
    NullPointer,
    InvalidArgument,
    InvalidDescriptor,
    DuplicateId,
    TooManyMods,
    UnknownHandle,
    NotPermitted,
    QueueFull,
    Panic,
    Unsupported,
    ManagerMissing,
    /// The request cannot be taken now: try again later.
    Busy,
    /// The game is not in the state the call needs.
    NotReady,
    /// Since 3.0. The manager is disabled: its tables stay readable, but every function refuses. A mod stops using
    /// the tables at their `ProviderGone`.
    Disabled,
}
