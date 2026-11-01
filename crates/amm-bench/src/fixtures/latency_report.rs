use std::time::Duration;

/// Percentiles of a set of durations.
pub struct LatencyReport {
    pub samples: usize,
    pub p50: Duration,
    pub p99: Duration,
    pub p999: Duration,
    pub max: Duration,
}

impl LatencyReport {
    /// The report of `durations`, which are sorted in place. An empty slice gives a zero report.
    pub fn new(durations: &mut [Duration]) -> Self {
        durations.sort_unstable();
        let last = durations.len().saturating_sub(1);
        let at =
            |fraction: f64| durations.get(((last as f64) * fraction).round() as usize).copied().unwrap_or_default();
        Self {
            samples: durations.len(),
            p50: at(0.50),
            p99: at(0.99),
            p999: at(0.999),
            max: durations.last().copied().unwrap_or_default(),
        }
    }
}
