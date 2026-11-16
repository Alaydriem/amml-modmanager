use amm_core::structs::input::{PointerButton, PointerOp, TurnAxis};
use amml_modmanager::services::{FakePointerSender, ManagerState};

use crate::support::log_capture::LogCapture;
use crate::support::recording_calls::{Call, RecordingCalls};

const DEVICE: usize = 0x9000;
// the address the `MouseDevice.instance` entry gave
const ENTRY_DEVICE: usize = 0x7700;

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

fn left(down: bool) -> PointerOp {
    PointerOp::Button { button: PointerButton::Left, down }
}

fn entry_feed(action: u8, data: i8, x: i16, y: i16) -> Call {
    Call::GameMouse { device: ENTRY_DEVICE, action, data, x, y }
}

#[test]
fn flush_tells_the_host_whether_the_feed_has_a_mouse_device() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakePointerSender::new();

    sender.flush(calls, RecordingCalls::CLIENT);
    let before = calls.pointer_feed();
    sender.set_device(ENTRY_DEVICE);
    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!((before, calls.pointer_feed()), (Some(false), Some(true)));
}

#[test]
fn a_device_from_the_entry_feeds_buttons_with_no_real_mouse_event() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[left(true), left(false)]);
    let sender = FakePointerSender::new();
    sender.set_device(ENTRY_DEVICE);

    sender.flush(calls, RecordingCalls::CLIENT);

    let button = FakePointerSender::LEFT_BUTTON;
    assert_eq!(calls.take(), vec![entry_feed(button, 1, 0, 0), entry_feed(button, 0, 0, 0)]);
    assert_eq!(calls.pending_fake_pointer(), vec![]);
}

#[test]
fn a_real_event_on_the_entry_device_gives_the_point() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[left(true)]);
    let sender = FakePointerSender::new();
    sender.set_device(ENTRY_DEVICE);
    sender.note(ENTRY_DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![entry_feed(FakePointerSender::LEFT_BUTTON, 1, 40, 30)]);
}

#[test]
fn a_real_event_on_another_device_does_not_replace_the_entry_device() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[left(true)]);
    let sender = FakePointerSender::new();
    sender.set_device(ENTRY_DEVICE);
    sender.note(DEVICE, 40, 30);

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![entry_feed(FakePointerSender::LEFT_BUTTON, 1, 0, 0)]);
}

#[test]
fn the_entry_device_fed_on_another_thread_than_the_tick_is_not_ready() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_pointer(&[left(false)]);
    let sender: &'static FakePointerSender = Box::leak(Box::new(FakePointerSender::new()));
    sender.set_device(ENTRY_DEVICE);
    std::thread::spawn(move || sender.note(ENTRY_DEVICE, 40, 30)).join().unwrap();

    sender.flush(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.pointer_feed(), Some(false));
    assert_eq!(calls.take(), vec![]);
}

#[test]
fn the_known_mouse_device_is_logged_once() {
    LogCapture::install();
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakePointerSender::new();

    sender.flush(calls, RecordingCalls::CLIENT);
    assert_eq!(LogCapture::count(FakePointerSender::DEVICE_LINE), 0);
    sender.set_device(ENTRY_DEVICE);
    for _ in 0..3 {
        sender.flush(calls, RecordingCalls::CLIENT);
    }

    assert_eq!(LogCapture::count(FakePointerSender::DEVICE_LINE), 1);
}
