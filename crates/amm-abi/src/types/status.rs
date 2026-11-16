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
    /// Since 2.4. The game is not in the state the call needs: `send_move` returns it while the game has no local
    /// player in a world, `send_key` for a key down while the manager does not yet know the game's keyboard
    /// controller, and `send_pointer` for a button down or a wheel turn while it does not yet know the game's mouse
    /// device.
    NotReady,
}
