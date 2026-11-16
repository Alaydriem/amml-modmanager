use amm_core::structs::input::MoveLayout;
use amml_modmanager::services::MoveLayoutSlots;

use crate::support::fake_hook_host::FakeHookHost;

// every move entry with the values the recipes derive on 1.26.63
fn entries() -> Vec<(&'static str, u64)> {
    vec![
        (MoveLayoutSlots::HOOK, 0x7ff6_1234_0000),
        ("RawMoveInputComponent.mInputState", 0),
        ("RawMoveInputComponent.mRawMove", 0x10),
        ("MoveInputState.mAnalogMoveVector", 4),
        ("MoveInputState.maskSneakTest", 0x40001),
        ("MoveInputState.maskJumpDown", 0x80),
        ("MoveInputState.bitSprintDown", 8),
        ("MoveInputState.bitUp", 13),
        ("MoveInputState.bitDown", 14),
        ("MoveInputState.bitLeft", 15),
        ("MoveInputState.bitRight", 16),
    ]
}

fn without(entry: &str) -> Vec<(&'static str, u64)> {
    entries().into_iter().filter(|(id, _)| *id != entry).collect()
}

#[test]
fn every_move_entry_resolved_gives_the_layout() {
    let host = FakeHookHost::new(&entries(), &[]);

    let layout = MoveLayoutSlots::resolve(&host);

    let expected = MoveLayout {
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
    };
    assert_eq!(layout, Some(expected));
    assert!(host.warnings.lock().unwrap().is_empty());
}

#[test]
fn a_missing_move_entry_turns_the_move_command_off_with_one_line() {
    for entry in MoveLayoutSlots::ENTRIES {
        let host = FakeHookHost::new(&without(entry), &[]);

        assert_eq!(MoveLayoutSlots::resolve(&host), None, "{entry}");

        let warnings = host.warnings.lock().unwrap().clone();
        assert_eq!(warnings.len(), 1, "{entry}");
        assert!(warnings[0].starts_with(entry) && warnings[0].ends_with("the move command is off"), "{warnings:?}");
    }
}

#[test]
fn a_layout_out_of_range_turns_the_move_command_off_with_one_line() {
    let mut bad = without("MoveInputState.bitUp");
    bad.push(("MoveInputState.bitUp", 40));
    let host = FakeHookHost::new(&bad, &[]);

    assert_eq!(MoveLayoutSlots::resolve(&host), None);

    let warnings = host.warnings.lock().unwrap().clone();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].ends_with("the move command is off"), "{warnings:?}");
}

#[test]
fn every_move_entry_is_in_the_entry_list() {
    let list = include_str!("../../packaging/amml.hcl");

    for entry in MoveLayoutSlots::ENTRIES {
        assert!(list.contains(&format!("\"{entry}\"")), "{entry} is not in amml.hcl");
    }
}

#[test]
fn an_unexpected_flag_number_keeps_the_layout_and_logs_once() {
    let mut moved = without("MoveInputState.bitSprintDown");
    moved.push(("MoveInputState.bitSprintDown", 9));
    let host = FakeHookHost::new(&moved, &[]);

    let layout = MoveLayoutSlots::resolve(&host);

    assert_eq!(layout.map(|layout| layout.sprint_bit), Some(9));
    let warnings = host.warnings.lock().unwrap().clone();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("flags"), "{warnings:?}");
}
