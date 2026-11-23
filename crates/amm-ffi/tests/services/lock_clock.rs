use std::time::Duration;

use amm_ffi::services::{LockClock, LockWindow};

#[test]
fn a_clock_with_telemetry_off_times_nothing_and_closes_no_window() {
    let clock = LockClock::new();

    assert!(clock.start().is_none());
    assert_eq!(clock.close_window(1), None);
    assert_eq!(clock.close_window(1 + LockClock::WINDOW_MS), None);
}

#[test]
fn a_window_gives_the_means_and_maxima_of_what_it_saw_then_starts_empty() {
    let clock = LockClock::new();
    clock.set_enabled(true);
    assert_eq!(clock.close_window(1_000), None);
    clock.record_hold(Duration::from_micros(100));
    clock.record_hold(Duration::from_micros(300));
    clock.record_wait(Duration::from_micros(50));

    let early = clock.close_window(1_000 + LockClock::WINDOW_MS - 1);
    let closed = clock.close_window(1_000 + LockClock::WINDOW_MS);
    let next = clock.close_window(1_000 + 2 * LockClock::WINDOW_MS);

    assert_eq!(early, None);
    assert_eq!(
        closed,
        Some(LockWindow {
            presents: 2,
            hold_mean_us: 200,
            hold_max_us: 300,
            waits: 1,
            wait_mean_us: 50,
            wait_max_us: 50
        })
    );
    assert_eq!(
        next,
        Some(LockWindow { presents: 0, hold_mean_us: 0, hold_max_us: 0, waits: 0, wait_mean_us: 0, wait_max_us: 0 })
    );
}

#[test]
fn turning_telemetry_off_and_on_starts_a_new_empty_window() {
    let clock = LockClock::new();
    clock.set_enabled(true);
    assert_eq!(clock.close_window(1_000), None);
    clock.record_hold(Duration::from_micros(900));

    clock.set_enabled(false);
    clock.set_enabled(true);
    let reopened = clock.close_window(1_000 + LockClock::WINDOW_MS);
    clock.record_hold(Duration::from_micros(100));
    let closed = clock.close_window(1_000 + 2 * LockClock::WINDOW_MS);

    assert_eq!(reopened, None);
    assert_eq!(closed.map(|window| (window.presents, window.hold_max_us)), Some((1, 100)));
}
