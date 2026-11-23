/// What `LockClock` measured over one window: the host lock hold time of each Present, and the time every other
/// caller waited to get the lock, as counts, means and maxima in microseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LockWindow {
    pub presents: u64,
    pub hold_mean_us: u64,
    pub hold_max_us: u64,
    pub waits: u64,
    pub wait_mean_us: u64,
    pub wait_max_us: u64,
}
