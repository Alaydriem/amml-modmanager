use std::sync::atomic::{AtomicU64, Ordering};

static COUNT: AtomicU64 = AtomicU64::new(0);

/// The count of the game's Presents that the overlay saw, for the real-time render debug work: the overlay calls
/// `next` once per outermost Present of any chain (a test Present does not count) and the host stamps the capture
/// stream frames it offers in that Present with `current`. Numbers start at 1; 0 means no Present yet. Any thread.
pub struct PresentSequence;

impl PresentSequence {
    /// A Present starts: its number.
    pub fn next() -> u64 {
        COUNT.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// The number of the newest Present started; during a Present on the render thread, that Present's number.
    pub fn current() -> u64 {
        COUNT.load(Ordering::SeqCst)
    }
}
