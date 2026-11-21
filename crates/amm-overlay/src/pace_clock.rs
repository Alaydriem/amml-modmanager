use std::sync::OnceLock;
use std::time::Instant;

// the instant the clock counts from; set at the first read
static ORIGIN: OnceLock<Instant> = OnceLock::new();

/// A monotonic clock for the pace measures, in microseconds. `Instant` reads the performance counter on Windows,
/// which is finer than `GetTickCount64`, whose steps of about 15 ms hide the interval of a game at 60 frames per
/// second.
pub(crate) struct PaceClock;

impl PaceClock {
    /// Microseconds from the first read of the clock to now.
    pub(crate) fn now_us() -> u64 {
        let origin = ORIGIN.get_or_init(Instant::now);
        u64::try_from(origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}
