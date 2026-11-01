use amm_overlay::pace_window::PaceWindow;

#[test]
fn summary_counts_presents_per_second() {
    let mut window = PaceWindow::new();
    for step in 0..=100u64 {
        window.add_present(step * 10_000, 2_000, false, Some(true));
    }
    let summary = window.summary();
    assert_eq!(summary.presents, 101);
    assert_eq!(summary.elapsed_ms, 1_000);
    assert!((summary.per_second - 101.0).abs() < 0.5, "per second {}", summary.per_second);
    assert_eq!(summary.present_ms_mean, 2.0);
    assert_eq!(summary.present_ms_max, 2.0);
    assert_eq!(summary.interval_ms_max, 10.0);
}

#[test]
fn sixty_per_second_at_a_16_667_ms_interval_is_not_biased() {
    let mut window = PaceWindow::new();
    window.reset(0);
    for step in 1..=60u64 {
        window.add_present(step * 16_667, 1_000, false, Some(true));
    }
    let summary = window.summary();
    assert!((summary.per_second - 60.0).abs() < 0.06, "per second {}", summary.per_second);
    assert_eq!(summary.interval_ms_max, 16.667);
}

#[test]
fn counts_occluded_and_foreground() {
    let mut window = PaceWindow::new();
    window.add_present(0, 100, false, Some(true));
    window.add_present(16_000, 100, true, Some(false));
    window.add_present(32_000, 100, false, Some(true));
    let summary = window.summary();
    assert_eq!(summary.presents, 3);
    assert_eq!(summary.occluded, 1);
    assert_eq!(summary.foreground, 2);
    assert_eq!(summary.unbound, 0);
}

#[test]
fn unbound_presents_are_neither_foreground_nor_background() {
    let mut window = PaceWindow::new();
    window.add_present(0, 100, false, None);
    window.add_present(16_000, 100, false, None);
    window.add_present(32_000, 100, false, Some(false));
    let summary = window.summary();
    assert_eq!(summary.presents, 3);
    assert_eq!(summary.unbound, 2);
    assert_eq!(summary.foreground, 0);
}

#[test]
fn readback_mean_and_max() {
    let mut window = PaceWindow::new();
    window.add_readback(3_000);
    window.add_readback(5_000);
    let summary = window.summary();
    assert_eq!(summary.readbacks, 2);
    assert_eq!(summary.readback_ms_mean, 4.0);
    assert_eq!(summary.readback_ms_max, 5.0);
}

#[test]
fn reset_clears_all() {
    let mut window = PaceWindow::new();
    window.add_present(0, 2_000, true, Some(true));
    window.add_present(10_000, 4_000, false, None);
    window.add_readback(3_000);
    window.reset(500_000);
    assert_eq!(window.summary(), PaceWindow::new().summary());
    window.add_present(510_000, 1_000, false, Some(false));
    let summary = window.summary();
    assert_eq!(summary.presents, 1);
    assert_eq!(summary.interval_ms_max, 0.0);
    assert_eq!(summary.elapsed_ms, 10);
}
