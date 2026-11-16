use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_core::structs::input::{MoveIntent, MoveLayout};
use amml_modmanager::game_hooks::extract_raw_hid_input;

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::RecordingCalls;

const UP: u32 = 1 << 13;
const DOWN: u32 = 1 << 14;
static ORIGINAL_CALLS: AtomicU32 = AtomicU32::new(0);

// the game fills the component from its keyboard: here, back held
extern "C" fn game_extract(_abilities: usize, _move_input: usize, _flags: usize, raw: usize, _e: usize, _f: usize) {
    ORIGINAL_CALLS.fetch_add(1, Ordering::SeqCst);
    // SAFETY: the test passes a live, aligned [u32; 6] as the component
    unsafe { (raw as *mut u32).write(DOWN) };
}

fn layout() -> MoveLayout {
    MoveLayout {
        input_state: 0,
        raw_move: 0x10,
        analog: 4,
        sneak_mask: 1,
        jump_mask: 0x80,
        sprint_bit: 8,
        up_bit: 13,
        down_bit: 14,
        left_bit: 15,
        right_bit: 16,
    }
}

#[test]
fn the_original_fills_the_component_first_then_the_held_intent_overwrites_it() {
    let (state, calls) = InstalledRuntime::install();
    extract_raw_hid_input::GUARD.original().store(game_extract as *mut c_void, Ordering::SeqCst);
    calls.set_in_play(true);
    *calls.move_intent.lock().unwrap() = Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO });
    state.fake_move().set_layout(Some(layout()));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = [0u32; 6];

    extract_raw_hid_input::detour(0x10, 0x20, 0x30, raw.as_mut_ptr() as usize, 0, 0);

    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(raw[0], UP);
    assert_eq!(f32::from_bits(raw[2]), 1.0);
}

#[test]
fn with_no_intent_the_component_keeps_what_the_game_wrote() {
    let (state, calls) = InstalledRuntime::install();
    extract_raw_hid_input::GUARD.original().store(game_extract as *mut c_void, Ordering::SeqCst);
    calls.set_in_play(true);
    state.fake_move().set_layout(Some(layout()));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = [0u32; 6];

    extract_raw_hid_input::detour(0x10, 0x20, 0x30, raw.as_mut_ptr() as usize, 0, 0);

    assert_eq!(raw, [DOWN, 0, 0, 0, 0, 0]);
}
