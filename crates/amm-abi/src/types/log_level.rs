/// The level of one log call.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmLogLevel {
    Error = 0,
    Warn,
    Info,
    Debug,
    Trace,
}
