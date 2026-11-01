use std::sync::atomic::Ordering;

use amm_overlay::PointerButton;
use amml_modmanager::services::{ManagerState, MouseInput};

use crate::support::recording_calls::{Call, RecordingCalls};

#[test]
fn a_move_reaches_the_host_as_its_delta() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let (mut x, mut y) = (100, 200);

    assert!(!MouseInput::feed(calls, 0, &mut x, &mut y, 3, -2));
    calls.consume_deltas.store(true, Ordering::SeqCst);
    assert!(MouseInput::feed(calls, 0, &mut x, &mut y, 3, -2));

    assert_eq!(calls.take(), vec![Call::PointerDelta(3, -2), Call::PointerDelta(3, -2)]);
    assert_eq!((x, y), (100, 200));
}

#[test]
fn a_button_the_window_router_consumed_is_cancelled() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.consumed_buttons.lock().unwrap().extend([PointerButton::Right, PointerButton::Wheel]);
    let (mut x, mut y) = (0, 0);

    let cancelled: Vec<bool> = (1..=4).map(|action| MouseInput::feed(calls, action, &mut x, &mut y, 0, 0)).collect();

    assert_eq!(cancelled, [false, true, false, true]);
    assert!(calls.take().is_empty(), "buttons do not go to the host again");
}

#[test]
fn inside_a_game_viewport_the_position_is_moved() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    *calls.game_pointer.lock().unwrap() = Some((640, 360));
    let (mut x, mut y) = (10, 20);

    MouseInput::feed(calls, 1, &mut x, &mut y, 0, 0);

    assert_eq!((x, y), (640, 360));
}
