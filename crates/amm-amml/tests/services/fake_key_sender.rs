use amm_core::structs::input::FakeKeyOp;
use amml_modmanager::services::{FakeKeySender, ManagerState};

use crate::support::log_capture::LogCapture;
use crate::support::recording_calls::{Call, RecordingCalls};

const SHIFT: u32 = 0x10;
const W: u32 = 0x57;

#[test]
fn flush_without_controller_keeps_ops() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let ops = [FakeKeyOp { vk: SHIFT, down: true }, FakeKeyOp { vk: W, down: true }];
    calls.queue_fake_keys(&ops);
    let sender = FakeKeySender::new();

    sender.flush(calls);

    assert_eq!(calls.take(), vec![]);
    assert_eq!(calls.pending_fake_keys(), ops.to_vec());
    assert_eq!(calls.key_feed(), Some(false));
}

#[test]
fn flush_sends_in_order_with_last_origin() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_keys(&[
        FakeKeyOp { vk: SHIFT, down: true },
        FakeKeyOp { vk: W, down: true },
        FakeKeyOp { vk: W, down: false },
    ]);
    let sender = FakeKeySender::new();
    sender.note(0x7000, 1);
    sender.note(0x7000, 2);

    sender.flush(calls);

    let key = |vk, down| Call::GameKey { controller: 0x7000, vk, down, origin: 2 };
    assert_eq!(calls.take(), vec![key(SHIFT, true), key(W, true), key(W, false)]);
    assert_eq!(calls.pending_fake_keys(), vec![]);
}

#[test]
fn flush_with_a_controller_tells_the_host_the_feed_is_ready() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakeKeySender::new();
    sender.note_controller(0x7100);

    sender.flush(calls);

    assert_eq!(calls.key_feed(), Some(true));
}

#[test]
fn a_controller_from_the_game_update_keeps_the_origin_of_the_last_real_key() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.queue_fake_keys(&[FakeKeyOp { vk: W, down: true }]);
    let sender = FakeKeySender::new();
    sender.note(0x7000, 2);
    sender.note_controller(0x7100);

    sender.flush(calls);

    assert_eq!(calls.take(), vec![Call::GameKey { controller: 0x7100, vk: W, down: true, origin: 2 }]);
}

#[test]
fn the_controller_from_the_game_update_is_logged_once() {
    LogCapture::install();
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakeKeySender::new();

    for _ in 0..3 {
        sender.note_controller(0x7100);
        sender.flush(calls);
    }

    assert_eq!(LogCapture::count(FakeKeySender::UPDATE_LINE), 1);
}

#[test]
fn the_game_update_is_logged_after_a_real_key_set_the_same_controller() {
    LogCapture::install();
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakeKeySender::new();
    sender.note(0x7000, 1);
    sender.flush(calls);

    sender.note_controller(0x7000);
    sender.flush(calls);
    sender.note_controller(0x7000);
    sender.flush(calls);

    assert_eq!(LogCapture::count(FakeKeySender::UPDATE_LINE), 1);
}

#[test]
fn without_the_game_update_nothing_claims_it() {
    LogCapture::install();
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let sender = FakeKeySender::new();
    sender.note(0x7000, 1);

    sender.flush(calls);

    assert_eq!(LogCapture::count(FakeKeySender::UPDATE_LINE), 0);
}
