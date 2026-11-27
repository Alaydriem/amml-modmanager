use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::LockWindow;

/// Times the host lock while the manager's telemetry is on: how long each Present holds it, and how long each other
/// caller (tick, input, table calls) waits for it. Fixed-size atomics, so recording never allocates or blocks; a
/// window closes at most once per `WINDOW_MS`.
pub struct LockClock {
    enabled: AtomicBool,
    presents: AtomicU64,
    hold_ns: AtomicU64,
    hold_max_ns: AtomicU64,
    waits: AtomicU64,
    wait_ns: AtomicU64,
    wait_max_ns: AtomicU64,
    // the `now_ms` the open window started at, 0 before the first
    window_start_ms: AtomicU64,
}

impl LockClock {
    pub const WINDOW_MS: u64 = 10_000;
    const NS_PER_US: u64 = 1000;

    pub fn new() -> Self {
        Self {
            enabled: AtomicBool::new(false),
            presents: AtomicU64::new(0),
            hold_ns: AtomicU64::new(0),
            hold_max_ns: AtomicU64::new(0),
            waits: AtomicU64::new(0),
            wait_ns: AtomicU64::new(0),
            wait_max_ns: AtomicU64::new(0),
            window_start_ms: AtomicU64::new(0),
        }
    }

    /// Turns timing on or off. Turning it off ends the open window, so the first window after it is on again
    /// starts empty.
    pub fn set_enabled(&self, enabled: bool) {
        if !enabled && self.enabled.swap(false, Ordering::Relaxed) {
            self.window_start_ms.store(0, Ordering::Relaxed);
            return;
        }
        self.enabled.store(enabled, Ordering::Relaxed);
    }

    /// The start of a timed span, or `None` while telemetry is off, so an untimed caller reads no clock.
    pub fn start(&self) -> Option<Instant> {
        self.enabled.load(Ordering::Relaxed).then(Instant::now)
    }

    /// Adds the host lock hold time of one Present.
    pub fn record_hold(&self, held: Duration) {
        Self::add(&self.presents, &self.hold_ns, &self.hold_max_ns, held);
    }

    /// Adds the time one caller waited for the host lock.
    pub fn record_wait(&self, waited: Duration) {
        Self::add(&self.waits, &self.wait_ns, &self.wait_max_ns, waited);
    }

    /// Closes the open window when `WINDOW_MS` passed since it started at `now_ms` and telemetry is on, and starts
    /// the next. `None` otherwise; the first call only starts a window.
    pub fn close_window(&self, now_ms: u64) -> Option<LockWindow> {
        if !self.enabled.load(Ordering::Relaxed) {
            return None;
        }
        let started = self.window_start_ms.load(Ordering::Relaxed);
        if started == 0 {
            self.window_start_ms.store(now_ms.max(1), Ordering::Relaxed);
            self.reset();
            return None;
        }
        if now_ms.saturating_sub(started) < Self::WINDOW_MS {
            return None;
        }
        self.window_start_ms.store(now_ms.max(1), Ordering::Relaxed);
        let presents = self.presents.swap(0, Ordering::Relaxed);
        let hold_ns = self.hold_ns.swap(0, Ordering::Relaxed);
        let hold_max_ns = self.hold_max_ns.swap(0, Ordering::Relaxed);
        let waits = self.waits.swap(0, Ordering::Relaxed);
        let wait_ns = self.wait_ns.swap(0, Ordering::Relaxed);
        let wait_max_ns = self.wait_max_ns.swap(0, Ordering::Relaxed);
        Some(LockWindow {
            presents,
            hold_mean_us: hold_ns.checked_div(presents).unwrap_or(0) / Self::NS_PER_US,
            hold_max_us: hold_max_ns / Self::NS_PER_US,
            waits,
            wait_mean_us: wait_ns.checked_div(waits).unwrap_or(0) / Self::NS_PER_US,
            wait_max_us: wait_max_ns / Self::NS_PER_US,
        })
    }

    fn reset(&self) {
        for counter in [&self.presents, &self.hold_ns, &self.hold_max_ns, &self.waits, &self.wait_ns, &self.wait_max_ns]
        {
            counter.store(0, Ordering::Relaxed);
        }
    }

    fn add(count: &AtomicU64, sum: &AtomicU64, max: &AtomicU64, span: Duration) {
        let ns = u64::try_from(span.as_nanos()).unwrap_or(u64::MAX);
        count.fetch_add(1, Ordering::Relaxed);
        sum.fetch_add(ns, Ordering::Relaxed);
        max.fetch_max(ns, Ordering::Relaxed);
    }
}

impl Default for LockClock {
    fn default() -> Self {
        Self::new()
    }
}
