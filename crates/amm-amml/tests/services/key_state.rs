use std::sync::atomic::Ordering;

use amml_modmanager::services::{KeyState, ManagerState};

use crate::support::recording_calls::{Call, RecordingCalls};

const VK_LSHIFT: i32 = 0xA0;
const VK_SHIFT: u32 = 0x10;
const VK_A: i32 = 0x41;

#[test]
fn left_shift_down_then_up_is_shift_down_and_up_with_no_repeat() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let keys = KeyState::new();

    keys.route(calls, VK_LSHIFT, true);
    keys.route(calls, VK_LSHIFT, false);

    assert_eq!(
        calls.take(),
        vec![
            Call::HostKey { vk: VK_SHIFT, down: true, repeat: false },
            Call::HostKey { vk: VK_SHIFT, down: false, repeat: false },
        ]
    );
}

#[test]
fn a_held_key_repeats_on_the_second_down() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let keys = KeyState::new();

    keys.route(calls, VK_A, true);
    keys.route(calls, VK_A, true);
    keys.route(calls, VK_A, false);
    keys.route(calls, VK_A, true);

    let repeats: Vec<bool> = calls
        .take()
        .into_iter()
        .filter_map(|call| match call {
            Call::HostKey { repeat, .. } => Some(repeat),
            _ => None,
        })
        .collect();
    assert_eq!(repeats, [false, true, false, false]);
}

#[test]
fn a_focus_loss_forgets_the_held_keys() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let keys = KeyState::new();
    keys.route(calls, VK_A, true);
    calls.take();
    calls.focus_lost.store(true, Ordering::SeqCst);

    keys.route(calls, VK_A, true);

    assert_eq!(calls.take(), vec![Call::HostKey { vk: 0x41, down: true, repeat: false }]);
}

#[test]
fn the_host_answer_decides_the_consume() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let keys = KeyState::new();
    assert!(!keys.route(calls, VK_A, true));
    calls.consume_keys.store(true, Ordering::SeqCst);
    assert!(keys.route(calls, VK_A, false));
}
