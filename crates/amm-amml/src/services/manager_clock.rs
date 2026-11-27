use std::time::Instant;

/// The manager's own clock, in milliseconds since the mod loaded. The host, the key router and the tick share one
/// clock so the times they give the host agree.
#[derive(Debug, Clone, Copy)]
pub struct ManagerClock {
    start: Instant,
}

impl ManagerClock {
    pub fn new() -> Self {
        Self { start: Instant::now() }
    }

    pub fn now_ms(&self) -> u64 {
        u64::try_from(self.start.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    /// The same clock in seconds, for the egui time of a frame.
    pub fn now_secs(&self) -> f64 {
        self.start.elapsed().as_secs_f64()
    }
}

impl Default for ManagerClock {
    fn default() -> Self {
        Self::new()
    }
}
