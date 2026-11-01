use crate::pace_summary::PaceSummary;

/// Fixed counters of the Presents and the readbacks of one stretch of time. Adding to it takes a few integer
/// operations and no allocation, so the render thread can do it on every Present. Times are in microseconds of
/// any one clock.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PaceWindow {
    // the clock value of the start of the window and of the last Present; `None` until the first Present or a reset
    start_us: Option<u64>,
    last_us: Option<u64>,
    presents: u32,
    present_us_sum: u64,
    present_us_max: u64,
    interval_us_max: u64,
    occluded: u32,
    foreground: u32,
    unbound: u32,
    readbacks: u32,
    readback_us_sum: u64,
    readback_us_max: u64,
}

impl PaceWindow {
    pub const fn new() -> Self {
        Self {
            start_us: None,
            last_us: None,
            presents: 0,
            present_us_sum: 0,
            present_us_max: 0,
            interval_us_max: 0,
            occluded: 0,
            foreground: 0,
            unbound: 0,
            readbacks: 0,
            readback_us_sum: 0,
            readback_us_max: 0,
        }
    }

    /// A Present at `now_us` that spent `inside_us` inside the original Present. `occluded` is set when it returned
    /// `DXGI_STATUS_OCCLUDED`. `foreground` is whether the game window was the foreground window, or `None` while
    /// no window is bound: such a Present counts as `unbound`, neither foreground nor background.
    pub fn add_present(&mut self, now_us: u64, inside_us: u64, occluded: bool, foreground: Option<bool>) {
        if self.start_us.is_none() {
            self.start_us = Some(now_us);
        }
        if let Some(last) = self.last_us {
            self.interval_us_max = self.interval_us_max.max(now_us.saturating_sub(last));
        }
        self.last_us = Some(now_us);
        self.presents = self.presents.saturating_add(1);
        self.present_us_sum = self.present_us_sum.saturating_add(inside_us);
        self.present_us_max = self.present_us_max.max(inside_us);
        self.occluded += u32::from(occluded);
        match foreground {
            Some(foreground) => self.foreground += u32::from(foreground),
            None => self.unbound += 1,
        }
    }

    /// A GPU readback that took `us`.
    pub fn add_readback(&mut self, us: u64) {
        self.readbacks = self.readbacks.saturating_add(1);
        self.readback_us_sum = self.readback_us_sum.saturating_add(us);
        self.readback_us_max = self.readback_us_max.max(us);
    }

    pub fn summary(&self) -> PaceSummary {
        let elapsed_us = match (self.start_us, self.last_us) {
            (Some(start), Some(last)) => last.saturating_sub(start),
            _ => 0,
        };
        PaceSummary {
            presents: self.presents,
            elapsed_ms: elapsed_us / 1_000,
            per_second: if elapsed_us == 0 { 0.0 } else { f64::from(self.presents) * 1_000_000.0 / elapsed_us as f64 },
            present_ms_mean: Self::mean_ms(self.present_us_sum, self.presents),
            present_ms_max: self.present_us_max as f64 / 1000.0,
            interval_ms_max: self.interval_us_max as f64 / 1000.0,
            occluded: self.occluded,
            foreground: self.foreground,
            unbound: self.unbound,
            readbacks: self.readbacks,
            readback_ms_mean: Self::mean_ms(self.readback_us_sum, self.readbacks),
            readback_ms_max: self.readback_us_max as f64 / 1000.0,
        }
    }

    /// Clears every counter; the window starts at `now_us`.
    pub fn reset(&mut self, now_us: u64) {
        *self = Self::new();
        self.start_us = Some(now_us);
    }

    fn mean_ms(sum_us: u64, count: u32) -> f64 {
        if count == 0 { 0.0 } else { sum_us as f64 / f64::from(count) / 1000.0 }
    }
}
