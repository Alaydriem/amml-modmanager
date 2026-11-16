use amm_core::structs::input::{MoveIntent, MoveLayout};
use amml_modmanager::services::{FakeMoveWriter, ManagerState};

use crate::support::recording_calls::{Call, RecordingCalls};

const UP: u32 = 1 << 13;
const JUMP: u32 = 0x80;
const SNEAK: u32 = 1;
// a "was pressed" edge bit and an unrelated bit the game set; neither is the writer's
const JUMP_EDGE: u32 = 1 << 25;
const OTHER: u32 = 1 << 20;

fn layout() -> MoveLayout {
    MoveLayout {
        input_state: 0,
        raw_move: 0x10,
        analog: 4,
        sneak_mask: 0x40001,
        jump_mask: 0x80,
        sprint_bit: 8,
        up_bit: 13,
        down_bit: 14,
        left_bit: 15,
        right_bit: 16,
    }
}

// a RawMoveInputComponent as the game filled it: the flag word, the analog vector, padding, the raw move
fn component(flags: u32) -> [u32; 6] {
    [flags, 0.5f32.to_bits(), 0.5f32.to_bits(), 0, 0.5f32.to_bits(), 0.5f32.to_bits()]
}

fn vectors(component: &[u32; 6]) -> ([f32; 2], [f32; 2]) {
    let f = |index: usize| f32::from_bits(component[index]);
    ([f(1), f(2)], [f(4), f(5)])
}

fn ready(intent: Option<MoveIntent>) -> (&'static ManagerState, &'static RecordingCalls) {
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    calls.set_in_play(true);
    *calls.move_intent.lock().unwrap() = intent;
    assert!(state.fake_move().set_layout(Some(layout())));
    (state, calls)
}

fn write(writer: &FakeMoveWriter, component: &mut [u32; 6]) {
    writer.write(component.as_mut_ptr() as usize);
}

#[test]
fn an_intent_lands_at_the_resolved_offsets_and_keeps_the_bits_it_does_not_own() {
    let walk = MoveIntent { forward: 1.0, jump: true, ..MoveIntent::ZERO };
    let (state, calls) = ready(Some(walk));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(SNEAK | JUMP_EDGE | OTHER);

    write(state.fake_move(), &mut raw);

    assert_eq!(raw[0], UP | JUMP | JUMP_EDGE | OTHER);
    assert_eq!(vectors(&raw), ([0.0, 1.0], [0.0, 1.0]));
    assert_eq!(raw[3], 0, "the padding between the vectors is not written");
}

#[test]
fn with_no_intent_the_component_is_untouched() {
    let (state, calls) = ready(None);
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(SNEAK | OTHER);

    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(SNEAK | OTHER));
}

#[test]
fn a_clear_stops_the_writes_at_the_next_tick_with_no_zero_write() {
    let (state, calls) = ready(Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO }));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    *calls.move_intent.lock().unwrap() = None;

    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(OTHER);
    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(OTHER));
}

#[test]
fn nothing_is_written_while_a_menu_is_shown() {
    let (state, calls) = ready(Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO }));
    calls.set_in_play(false);
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(0);

    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(0));
    assert!(calls.take().contains(&Call::HostMove { player_ready: true }));
}

#[test]
fn with_no_local_player_the_host_hears_not_ready_and_nothing_is_written() {
    let (state, calls) = ready(Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO }));
    calls.local_player.store(0, std::sync::atomic::Ordering::SeqCst);
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(0);

    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(0));
    assert!(calls.take().contains(&Call::HostMove { player_ready: false }));
}

#[test]
fn without_a_layout_the_host_is_not_asked_and_nothing_is_written() {
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    calls.set_in_play(true);
    *calls.move_intent.lock().unwrap() = Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO });

    assert!(!state.fake_move().set_layout(None));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(0);
    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(0));
    assert!(calls.take().iter().all(|call| !matches!(call, Call::HostMove { .. })));
}

#[test]
fn a_layout_that_would_write_past_the_component_is_not_kept() {
    let state = ManagerState::leaked();

    assert!(!state.fake_move().set_layout(Some(MoveLayout { raw_move: 0x14, ..layout() })));
}

#[test]
fn stop_drops_the_published_intent() {
    let (state, calls) = ready(Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO }));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);

    state.fake_move().stop();
    let mut raw = component(0);
    write(state.fake_move(), &mut raw);

    assert_eq!(raw, component(0));
}

#[test]
fn a_null_component_is_not_written() {
    let (state, calls) = ready(Some(MoveIntent { forward: 1.0, ..MoveIntent::ZERO }));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);

    state.fake_move().write(0);
}

#[test]
fn with_unexpected_flag_numbers_only_the_vectors_are_written() {
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    calls.set_in_play(true);
    *calls.move_intent.lock().unwrap() = Some(MoveIntent { forward: 1.0, jump: true, ..MoveIntent::ZERO });
    assert!(state.fake_move().set_layout(Some(MoveLayout { jump_mask: 0x200, ..layout() })));
    state.fake_move().publish(calls, RecordingCalls::CLIENT);
    let mut raw = component(SNEAK | OTHER);

    write(state.fake_move(), &mut raw);

    assert_eq!(raw[0], SNEAK | OTHER);
    assert_eq!(vectors(&raw), ([0.0, 1.0], [0.0, 1.0]));
}
