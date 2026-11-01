use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use crate::in_flight_call::InFlightCall;

/// Counts the threads inside an overlay detour, so shutdown can wait until none of them runs overlay code.
pub(crate) struct InFlight {
    count: AtomicUsize,
}

impl InFlight {
    const POLL: Duration = Duration::from_millis(1);

    pub(crate) const fn new() -> Self {
        Self { count: AtomicUsize::new(0) }
    }

    /// Counts the calling thread until the returned guard drops.
    pub(crate) fn enter(&self) -> InFlightCall<'_> {
        self.count.fetch_add(1, Ordering::SeqCst);
        InFlightCall::new(self)
    }

    pub(crate) fn leave(&self) {
        self.count.fetch_sub(1, Ordering::SeqCst);
    }

    /// Waits up to `timeout` for the count to reach zero. False when a thread is still inside at the deadline.
    pub(crate) fn wait_drained(&self, timeout: Duration) -> bool {
        self.wait_until_at_most(0, timeout)
    }

    /// Waits up to `timeout` for the count to fall to `own` or less, where `own` is the calls of the waiting
    /// thread itself. False when other threads are still inside at the deadline.
    pub(crate) fn wait_until_at_most(&self, own: usize, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while self.count.load(Ordering::SeqCst) > own {
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(Self::POLL);
        }
        true
    }
}
