use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use amm_ffi::host::amm_host_key;

use super::{BenchHost, ComposeMode, ComposeRig, LatencyReport};

/// Measures how long `amm_host_key` takes to return while another thread composes frames under the host lock.
pub struct RouteLatency;

/// The two reports of one run: the key calls, and the composes that ran meanwhile.
pub struct RouteRun {
    pub keys: LatencyReport,
    pub composes: LatencyReport,
}

impl RouteRun {
    /// One printed line: the key percentiles, then the compose percentiles, in microseconds.
    pub fn line(&self, name: &str, case: &str, mode: &str) -> String {
        let us = |duration: Duration| duration.as_secs_f64() * 1e6;
        let (k, c) = (&self.keys, &self.composes);
        format!(
            "{name}/{case}/{mode}: key p50 {:.1} us, p99 {:.1} us, p99.9 {:.1} us, max {:.1} us ({} keys) | \n             compose p50 {:.1} us, p99 {:.1} us, p99.9 {:.1} us, max {:.1} us ({} composes)",
            us(k.p50),
            us(k.p99),
            us(k.p999),
            us(k.max),
            k.samples,
            us(c.p50),
            us(c.p99),
            us(c.p999),
            us(c.max),
            c.samples,
        )
    }
}

impl RouteLatency {
    const VK_Q: u32 = 0x51;
    const KEY_GAP: Duration = Duration::from_micros(500);
    const FRAME_PERIOD: Duration = Duration::from_micros(16_667);
    const SETTLE: Duration = Duration::from_millis(200);
    // Bound on the compose durations kept; a run past it keeps composing but records no more.
    const MAX_COMPOSES: usize = 200_000;

    /// Sends `samples` key events, 500 us apart, and times each call.
    pub fn run(rig: &ComposeRig, mode: ComposeMode, samples: usize) -> RouteRun {
        let stop = AtomicBool::new(false);
        let recording = AtomicBool::new(false);
        let host = rig.host();
        let (mut keys, mut composes) = thread::scope(|scope| {
            let compose =
                (mode != ComposeMode::None).then(|| scope.spawn(|| Self::compose_loop(host, mode, &stop, &recording)));
            // Let the compose thread reach its steady state before the first sample.
            thread::sleep(Self::SETTLE);
            recording.store(true, Ordering::SeqCst);
            let keys = Self::key_loop(samples);
            stop.store(true, Ordering::SeqCst);
            let composes = compose
                .map(|handle| handle.join().unwrap_or_else(|_| panic!("the compose thread panicked")))
                .unwrap_or_default();
            (keys, composes)
        });
        assert!(mode == ComposeMode::None || !composes.is_empty(), "no compose ran during the measured keys");
        RouteRun { keys: LatencyReport::new(&mut keys), composes: LatencyReport::new(&mut composes) }
    }

    fn key_loop(samples: usize) -> Vec<Duration> {
        let mut durations = Vec::with_capacity(samples);
        let mut next = Instant::now();
        for n in 0..samples {
            next += Self::KEY_GAP;
            while Instant::now() < next {
                std::hint::spin_loop();
            }
            let down = n % 2 == 0;
            let started = Instant::now();
            std::hint::black_box(amm_host_key(Self::VK_Q, down, false, 0, n as u64));
            durations.push(started.elapsed());
        }
        durations
    }

    fn compose_loop(host: &BenchHost, mode: ComposeMode, stop: &AtomicBool, recording: &AtomicBool) -> Vec<Duration> {
        let mut durations = Vec::with_capacity(Self::MAX_COMPOSES);
        let mut now_ms = 0u64;
        let mut next = Instant::now();
        while !stop.load(Ordering::SeqCst) {
            now_ms += 16;
            let started = Instant::now();
            std::hint::black_box(host.frame(now_ms));
            if recording.load(Ordering::SeqCst) && durations.len() < Self::MAX_COMPOSES {
                durations.push(started.elapsed());
            }
            if mode == ComposeMode::Paced {
                next += Self::FRAME_PERIOD;
                let now = Instant::now();
                if next > now {
                    thread::sleep(next - now);
                } else {
                    next = now;
                }
            }
        }
        durations
    }
}
