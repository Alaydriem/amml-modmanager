/// What `ClientFlags` knows about one client flag: whether the manager's value is written (the HUD hidden, the
/// mouse released), whether the flag is off for good, and whether a refusal of it was logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlagState {
    pub applied: bool,
    pub off: bool,
    pub refusal_logged: bool,
}
