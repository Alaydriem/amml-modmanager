use std::sync::{Mutex, MutexGuard, PoisonError};

use windows::Win32::Foundation::DXGI_STATUS_OCCLUDED;
use windows::core::HRESULT;

use crate::pace_clock::PaceClock;
use crate::pace_state::PaceState;
use crate::pace_summary::PaceSummary;

/// Log-only: how the game presents. It counts every outermost Present, the time spent inside the original Present
/// (a vsync wait shows there), the Presents the DXGI reports as occluded, the Presents made while the game window is
/// in front, and the time of the GPU readbacks. Once per second, at debug level, it logs the window; it logs the
/// `SyncInterval` and flags whenever they change; and when a forced render size ends it logs the totals of the span.
/// It changes nothing in the Present. A render that runs no faster at 1080p than at 4K, with `SyncInterval` 1, is
/// held by vsync.
pub(crate) struct PresentPace {
    state: Mutex<PaceState>,
}

impl PresentPace {
    const WINDOW_MS: u64 = 1_000;

    pub(crate) const fn new() -> Self {
        Self { state: Mutex::new(PaceState::new()) }
    }

    /// The host forces the chain's size (`true`) or lets the game choose it (`false`). A new forced span starts with
    /// fresh counters; the end of a span logs its totals.
    pub(crate) fn set_forced(&self, forced: bool) {
        let ended = {
            let mut state = self.lock();
            if state.forced == forced {
                return;
            }
            state.forced = forced;
            if forced {
                state.span = Default::default();
                state.seen_len = 0;
                return;
            }
            (state.span.summary(), state.seen, state.seen_len)
        };
        let (summary, seen, seen_len) = ended;
        let (sync_intervals, flags) = Self::seen_values(&seen[..seen_len]);
        curia::info!("present pace while forced", {
            presents: summary.presents,
            elapsed_ms: summary.elapsed_ms,
            per_second: summary.per_second,
            present_ms_mean: summary.present_ms_mean,
            present_ms_max: summary.present_ms_max,
            interval_ms_max: summary.interval_ms_max,
            occluded: summary.occluded,
            foreground: summary.foreground,
            unbound: summary.unbound,
            readbacks: summary.readbacks,
            readback_ms_mean: summary.readback_ms_mean,
            readback_ms_max: summary.readback_ms_max,
            sync_intervals: sync_intervals,
            flags: flags
        });
    }

    /// A finished outermost, non-test Present with `sync` and `flags` that spent `inside_us` inside the original
    /// and returned `result`; `foreground` is whether the game window was the foreground window, `None` while no window is bound. A Present that
    /// does not `count` is not seen.
    pub(crate) fn observe(
        &self,
        sync: u32,
        flags: u32,
        result: HRESULT,
        inside_us: u64,
        counts: bool,
        foreground: Option<bool>,
    ) {
        if !counts {
            return;
        }
        let key = (u64::from(sync) << 32) | u64::from(flags);
        let now = PaceClock::now_us();
        let occluded = result == DXGI_STATUS_OCCLUDED;
        let (changed, report) = {
            let mut state = self.lock();
            let mut changed = false;
            if state.last_key.replace(key) != Some(key) {
                state.sync_changes = state.sync_changes.saturating_add(1);
                // one line per counting second; the rest show as `sync_changes` of the `present pace` line
                changed = !std::mem::replace(&mut state.change_logged, true);
            }
            if state.forced {
                state.see(key);
                state.span.add_present(now, inside_us, occluded, foreground);
            }
            state.window.add_present(now, inside_us, occluded, foreground);
            let summary = state.window.summary();
            let report = if summary.elapsed_ms >= Self::WINDOW_MS {
                state.window.reset(now);
                let changes = std::mem::take(&mut state.sync_changes);
                state.change_logged = false;
                Some((summary, state.forced, changes))
            } else {
                None
            };
            (changed, report)
        };
        if changed {
            curia::info!("present sync changed", { sync_interval: sync, flags: flags });
        }
        if let Some((summary, forced, changes)) = report {
            Self::log_window(&summary, forced, changes, sync, flags);
        }
    }

    /// A GPU readback that took `us`.
    pub(crate) fn add_readback(&self, us: u64) {
        let mut state = self.lock();
        state.window.add_readback(us);
        if state.forced {
            state.span.add_readback(us);
        }
    }

    fn log_window(summary: &PaceSummary, forced: bool, changes: u32, sync: u32, flags: u32) {
        curia::debug!("present pace", {
            presents: summary.presents,
            elapsed_ms: summary.elapsed_ms,
            per_second: summary.per_second,
            present_ms_mean: summary.present_ms_mean,
            present_ms_max: summary.present_ms_max,
            interval_ms_max: summary.interval_ms_max,
            occluded: summary.occluded,
            foreground: summary.foreground,
            unbound: summary.unbound,
            readbacks: summary.readbacks,
            readback_ms_mean: summary.readback_ms_mean,
            readback_ms_max: summary.readback_ms_max,
            forced: u32::from(forced),
            sync_changes: changes,
            sync_interval: sync,
            flags: flags
        });
    }

    // the distinct sync intervals and the distinct flags of `keys`, each as a comma separated list
    fn seen_values(keys: &[u64]) -> (String, String) {
        let mut syncs: Vec<u32> = Vec::new();
        let mut flags: Vec<u32> = Vec::new();
        for &key in keys {
            let (sync, flag) = ((key >> 32) as u32, key as u32);
            if !syncs.contains(&sync) {
                syncs.push(sync);
            }
            if !flags.contains(&flag) {
                flags.push(flag);
            }
        }
        let join = |values: &[u32]| values.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        (join(&syncs), join(&flags))
    }

    // a panic in a detour is contained, so a poisoned lock still holds counters worth reading
    fn lock(&self) -> MutexGuard<'_, PaceState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
