use amm_core::structs::input::FakeKeyOp;
use amml_modmanager::services::{FakeKeySender, ManagerState};

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
