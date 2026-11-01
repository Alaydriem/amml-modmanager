/// How an acquire call ended.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmAcquireOutcome {
    Granted = 0,
    Denied,
    Preempted,
}
