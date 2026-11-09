use amm_core::structs::input::{PointerButton, PointerOp, TurnAxis};
use amml_modmanager::services::{FakePointerSender, ManagerState};

use crate::support::recording_calls::{Call, RecordingCalls};

const DEVICE: usize = 0x9000;

fn feed(action: u8, data: i8) -> Call {
    Call::GameMouse { device: DEVICE, action, data, x: 40, y: 30 }
}

#[test]
fn flush_feeds_the_original_mouse_device_with_button_and_wheel() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[
        PointerOp::Button { button: PointerButton::Left, down: true },
        PointerOp::Wheel { steps: 2 },
        PointerOp::Wheel { steps: -1 },
        PointerOp::Button { button: PointerButton::Left, down: false },
    ]);
    let sender = FakePointerSender::new();
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    let notch = FakePointerSender::WHEEL_NOTCH;
    assert_eq!(
        calls.take(),
        vec![
            feed(FakePointerSender::LEFT_BUTTON, 1),
            feed(FakePointerSender::WHEEL_BUTTON, notch),
            feed(FakePointerSender::WHEEL_BUTTON, notch),
            feed(FakePointerSender::WHEEL_BUTTON, -notch),
            feed(FakePointerSender::LEFT_BUTTON, 0),
        ]
    );
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

#[test]
fn flush_maps_right_and_middle_to_the_game_button_ids() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[
        PointerOp::Button { button: PointerButton::Right, down: true },
        PointerOp::Button { button: PointerButton::Middle, down: true },
    ]);
    let sender = FakePointerSender::new();
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![feed(FakePointerSender::RIGHT_BUTTON, 1), feed(FakePointerSender::MIDDLE_BUTTON, 1)]);
}

#[test]
fn flush_without_a_mouse_device_drops_the_ops_so_none_arrive_late() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[PointerOp::Button { button: PointerButton::Left, down: true }]);
    let sender = FakePointerSender::new();

    sender.flush(calls, RecordingCalls::CLIENT);
    sender.note(DEVICE, 40, 30);
    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![]);
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

#[test]
fn flush_on_another_thread_than_the_mouse_feed_drops_the_ops() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[PointerOp::Button { button: PointerButton::Left, down: true }]);
    let sender: &'static FakePointerSender = Box::leak(Box::new(FakePointerSender::new()));
    std::thread::spawn(move || sender.note(DEVICE, 40, 30)).join().unwrap();

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![]);
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

fn turn(axis: TurnAxis, millidegrees: i32) -> PointerOp {
    PointerOp::Turn { axis, millidegrees }
}

#[test]
fn the_sender_applies_the_pending_turn_through_the_original_once() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.set_in_play(true);
    calls.queue_fake_pointer(&[turn(TurnAxis::Pitch, -2500), turn(TurnAxis::Yaw, 90_000), turn(TurnAxis::Yaw, 500)]);
    let sender = FakePointerSender::new();
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(
        calls.take(),
        vec![Call::GameTurn { player: RecordingCalls::PLAYER, pitch_millidegrees: -2500, yaw_millidegrees: 90_500 }]
    );
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

#[test]
fn a_turn_needs_no_mouse_device_and_does_not_disturb_the_buttons() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.set_in_play(true);
    calls.queue_fake_pointer(&[
        PointerOp::Button { button: PointerButton::Left, down: true },
        turn(TurnAxis::Yaw, 1000),
    ]);
    let sender = FakePointerSender::new();
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(
        calls.take(),
        vec![
            Call::GameTurn { player: RecordingCalls::PLAYER, pitch_millidegrees: 0, yaw_millidegrees: 1000 },
            feed(FakePointerSender::LEFT_BUTTON, 1),
        ]
    );
}

#[test]
fn no_turn_is_applied_without_a_local_player() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[turn(TurnAxis::Yaw, 1000)]);
    let sender = FakePointerSender::new();
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![]);
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

#[test]
fn a_turn_without_a_client_is_dropped() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.set_in_play(true);
    calls.queue_fake_pointer(&[turn(TurnAxis::Yaw, 1000)]);
    let sender = FakePointerSender::new();

    sender.flush(calls, 0);

    assert_eq!(calls.take(), vec![]);
}
