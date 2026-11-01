use amm_abi::types::AmmCursorShape;
use amml_modmanager::services::{FrameState, StopLatch};

#[test]
fn the_safe_state_is_published_once_until_a_frame_runs_again() {
    let latch = StopLatch::new();
    let state = FrameState::new();
    state.publish(&latch, true, AmmCursorShape::Hand, true);

    assert!(state.publish_safe(&latch));
    assert!(!state.cursor_free());
    assert!(!state.hide_hud());
    assert!(!state.publish_safe(&latch), "a second skipped frame publishes nothing");

    state.publish(&latch, true, AmmCursorShape::Text, false);
    assert_eq!((state.cursor_free(), state.cursor()), (true, AmmCursorShape::Text));
    assert!(state.publish_safe(&latch));
}

#[test]
fn nothing_is_published_while_the_latch_is_raised() {
    let latch = StopLatch::new();
    let state = FrameState::new();
    state.publish(&latch, true, AmmCursorShape::Hand, true);
    latch.raise();

    state.publish(&latch, false, AmmCursorShape::Default, false);
    assert!(!state.publish_safe(&latch));

    assert!(state.cursor_free());
    assert!(state.hide_hud());
}
