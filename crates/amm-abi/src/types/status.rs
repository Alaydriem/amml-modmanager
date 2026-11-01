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
    /// The request cannot be taken now: try again later. `request_screenshot` and `request_screenshot_layers`
    /// return it while a capture of the mod is in flight, or while every capture slot of the manager is in use.
    Busy,
}
