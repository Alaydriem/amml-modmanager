use amm_core::structs::input::{MoveIntent, MoveLayout};

const SNEAK: u32 = 1;
const JUMP: u32 = 0x80;
const SPRINT: u32 = 1 << 8;
const UP: u32 = 1 << 13;
const DOWN: u32 = 1 << 14;
const LEFT: u32 = 1 << 15;
const RIGHT: u32 = 1 << 16;
const OWNED: u32 = SNEAK | JUMP | SPRINT | UP | DOWN | LEFT | RIGHT;

// the values the recipes derive on 1.26.62 and 1.26.63
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

fn intent(forward: f32, left: f32) -> MoveIntent {
    MoveIntent { forward, left, ..MoveIntent::ZERO }
}

#[test]
fn the_intent_maps_to_move_input_state_flags() {
    let layout = layout().checked().unwrap_or_else(|| panic!("the derived layout is valid"));

    let forward = layout.write_for(intent(1.0, 0.0));
    assert_eq!(forward.set, UP);
    assert_eq!(forward.vector, [0.0, 1.0]);

    let left = layout.write_for(intent(0.0, 1.0));
    assert_eq!(left.set, LEFT);
    assert_eq!(left.vector, [1.0, 0.0]);

    let back_right = layout.write_for(intent(-1.0, -1.0));
    assert_eq!(back_right.set, DOWN | RIGHT);

    let actions = layout.write_for(MoveIntent { jump: true, sneak: true, sprint: true, ..MoveIntent::ZERO });
    assert_eq!(actions.set, JUMP | SNEAK | SPRINT);
    assert_eq!(actions.vector, [0.0, 0.0]);
}

#[test]
fn the_write_clears_only_the_held_bits_it_owns() {
    let layout = layout().checked().unwrap_or_else(|| panic!("the derived layout is valid"));

    let write = layout.write_for(intent(1.0, 0.0));

    assert_eq!(write.clear, OWNED);
    assert_eq!(write.clear & (1 << 18), 0, "the Descend bit of the sneak mask is not ours");
    assert_eq!(write.clear & (1 << 25), 0, "the jump edge bit is never written");
}

#[test]
fn a_diagonal_is_held_to_length_one() {
    let layout = layout().checked().unwrap_or_else(|| panic!("the derived layout is valid"));

    let write = layout.write_for(intent(1.0, 1.0));

    let length = (write.vector[0] * write.vector[0] + write.vector[1] * write.vector[1]).sqrt();
    assert!((length - 1.0).abs() < 1e-6, "length {length}");
    assert_eq!(write.set, UP | LEFT);
}

#[test]
fn a_small_axis_moves_the_vector_but_sets_no_direction_bit() {
    let layout = layout().checked().unwrap_or_else(|| panic!("the derived layout is valid"));

    let write = layout.write_for(intent(0.2, -0.2));

    assert_eq!(write.set, 0);
    assert_eq!(write.vector, [-0.2, 0.2]);
}

#[test]
fn a_layout_that_would_write_past_the_component_is_refused() {
    assert!(MoveLayout { raw_move: MoveLayout::COMPONENT_SIZE - 4, ..layout() }.checked().is_none());
    assert!(MoveLayout { analog: 0x10, ..layout() }.checked().is_none());
    assert!(MoveLayout { input_state: 0x18, ..layout() }.checked().is_none());
}

#[test]
fn a_layout_with_a_bad_flag_is_refused() {
    assert!(MoveLayout { up_bit: 32, ..layout() }.checked().is_none());
    assert!(MoveLayout { jump_mask: 0, ..layout() }.checked().is_none());
    assert!(MoveLayout { sneak_mask: 0, ..layout() }.checked().is_none());
    assert!(MoveLayout { left_bit: 13, ..layout() }.checked().is_none());
    assert!(MoveLayout { sprint_bit: 7, ..layout() }.checked().is_none());
}

#[test]
fn a_flag_number_other_than_expected_writes_only_the_vector() {
    let moved = MoveLayout { up_bit: 12, ..layout() };
    assert!(!moved.flags_as_expected());
    let moved = moved.checked().unwrap_or_else(|| panic!("still a valid layout"));

    let write = moved.write_for(MoveIntent { forward: 1.0, jump: true, sneak: true, ..MoveIntent::ZERO });

    assert_eq!((write.clear, write.set), (0, 0));
    assert_eq!(write.vector, [0.0, 1.0]);
    assert!(layout().flags_as_expected());
    assert!(!MoveLayout { jump_mask: 0x100, sprint_bit: 9, ..layout() }.flags_as_expected());
    assert!(!MoveLayout { sneak_mask: 0x40000, ..layout() }.flags_as_expected(), "a sneak test without SneakDown");
}
