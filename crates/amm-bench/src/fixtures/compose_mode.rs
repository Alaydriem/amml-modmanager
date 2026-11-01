/// How the compose thread runs while keys are measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposeMode {
    /// No compose thread: the baseline.
    None,
    /// One compose per 60 Hz frame period.
    Paced,
    /// The next compose starts as the last one ends, the worst case for the lock.
    BackToBack,
}
