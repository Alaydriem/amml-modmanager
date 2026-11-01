use std::sync::atomic::{AtomicI64, Ordering};

use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};

// the counter frequency in ticks per second; 0 until the first read
static FREQUENCY: AtomicI64 = AtomicI64::new(0);

/// The performance counter as a clock for the pace measures: finer than `GetTickCount64`, whose steps of about
/// 15 ms hide the interval of a game at 60 frames per second.
pub(crate) struct PaceClock;

impl PaceClock {
    /// The counter now, in ticks.
    pub(crate) fn ticks() -> i64 {
        let mut counter = 0i64;
        // SAFETY: `counter` is writable for the call; the call cannot fail on Windows XP or later
        let _ = unsafe { QueryPerformanceCounter(&mut counter) };
        counter
    }

    /// Microseconds from `start`, a value of `ticks`, to now.
    pub(crate) fn micros_since(start: i64) -> u64 {
        let ticks = Self::ticks().saturating_sub(start).max(0) as u128;
        (ticks * 1_000_000 / Self::frequency() as u128) as u64
    }

    /// The counter now, in microseconds.
    pub(crate) fn now_us() -> u64 {
        (Self::ticks().max(0) as u128 * 1_000_000 / Self::frequency() as u128) as u64
    }

    fn frequency() -> i64 {
        let cached = FREQUENCY.load(Ordering::Relaxed);
        if cached > 0 {
            return cached;
        }
        let mut frequency = 0i64;
        // SAFETY: `frequency` is writable for the call
        let _ = unsafe { QueryPerformanceFrequency(&mut frequency) };
        let frequency = frequency.max(1);
        FREQUENCY.store(frequency, Ordering::Relaxed);
        frequency
    }
}
