use amm_core::services::input::FakePointer;
use amm_core::structs::input::{PointerButton, PointerOp, TurnAxis};
use amm_core::structs::registry::ModIndex;

fn ops(pointer: &mut FakePointer) -> Vec<PointerOp> {
    let mut out = Vec::new();
    pointer.take_ops(&mut out);
    out
}

fn down(button: PointerButton) -> PointerOp {
    PointerOp::Button { button, down: true }
}

fn up(button: PointerButton) -> PointerOp {
    PointerOp::Button { button, down: false }
}

#[test]
fn fake_pointer_queue_drops_the_oldest_past_capacity() {
    let mut pointer = FakePointer::new();

    for steps in 1..=(FakePointer::CAPACITY as i32 + 1) {
        pointer.wheel(steps);
    }

    assert_eq!(pointer.pending(), FakePointer::CAPACITY);
    assert_eq!(pointer.take_dropped(), 1);
    assert_eq!(pointer.take_dropped(), 0);
    let queued = ops(&mut pointer);
    assert_eq!(queued.len(), FakePointer::CAPACITY);
    assert_eq!(queued[0], PointerOp::Wheel { steps: 2 });
    assert_eq!(queued[FakePointer::CAPACITY - 1], PointerOp::Wheel { steps: FakePointer::CAPACITY as i32 + 1 });
}

#[test]
fn a_button_goes_down_with_the_first_holder_and_up_with_the_last() {
    let mut pointer = FakePointer::new();
    let (a, b) = (ModIndex(0), ModIndex(1));

    assert!(pointer.hold(a, PointerButton::Left));
    assert!(pointer.hold(b, PointerButton::Left));
    assert!(pointer.release(a, PointerButton::Left));
    assert!(pointer.is_held(PointerButton::Left));
    assert_eq!(ops(&mut pointer), vec![down(PointerButton::Left)]);

    assert!(pointer.release(b, PointerButton::Left));

    assert!(!pointer.is_held(PointerButton::Left));
    assert_eq!(ops(&mut pointer), vec![up(PointerButton::Left)]);
}

#[test]
fn holding_a_held_button_or_releasing_a_free_one_queues_nothing() {
    let mut pointer = FakePointer::new();
    let m = ModIndex(2);
    pointer.hold(m, PointerButton::Right);
    ops(&mut pointer);

    assert!(!pointer.hold(m, PointerButton::Right));
    assert!(!pointer.release(m, PointerButton::Middle));

    assert!(ops(&mut pointer).is_empty());
}

#[test]
fn release_all_except_keeps_the_kept_mods_buttons() {
    let mut pointer = FakePointer::new();
    let (keep, other) = (ModIndex(0), ModIndex(1));
    pointer.hold(keep, PointerButton::Left);
    pointer.hold(other, PointerButton::Right);
    ops(&mut pointer);

    let lost = pointer.release_all_except(keep);

    assert!(lost.contains(other) && !lost.contains(keep));
    assert!(pointer.is_held(PointerButton::Left));
    assert_eq!(ops(&mut pointer), vec![up(PointerButton::Right)]);
}

#[test]
fn release_mod_lets_go_of_every_button_of_the_mod() {
    let mut pointer = FakePointer::new();
    let m = ModIndex(3);
    pointer.hold(m, PointerButton::Left);
    pointer.hold(m, PointerButton::Middle);
    ops(&mut pointer);

    assert!(pointer.release_mod(m).contains(m));

    assert_eq!(ops(&mut pointer), vec![up(PointerButton::Left), up(PointerButton::Middle)]);
    assert!(pointer.release_mod(m).is_empty());
}

#[test]
fn a_full_queue_evicts_wheel_ops_and_keeps_a_button_up() {
    let mut pointer = FakePointer::new();
    let m = ModIndex(0);
    pointer.hold(m, PointerButton::Left);
    for _ in 0..FakePointer::CAPACITY + 10 {
        assert!(pointer.wheel(1));
    }

    assert!(pointer.release(m, PointerButton::Left));

    let queued = ops(&mut pointer);
    assert_eq!(queued.len(), FakePointer::CAPACITY);
    assert_eq!(queued[0], down(PointerButton::Left));
    assert_eq!(queued[FakePointer::CAPACITY - 1], up(PointerButton::Left));
    assert!(!pointer.is_held(PointerButton::Left));
}

#[test]
fn a_queue_full_of_button_ops_refuses_the_next_op() {
    let mut pointer = FakePointer::new();
    let m = ModIndex(0);
    for _ in 0..FakePointer::CAPACITY / 2 {
        pointer.hold(m, PointerButton::Left);
        pointer.release(m, PointerButton::Left);
    }
    pointer.take_dropped();

    assert!(!pointer.has_room());
    assert!(!pointer.hold(m, PointerButton::Right));
    assert!(!pointer.wheel(1));

    assert!(!pointer.is_held(PointerButton::Right));
    assert_eq!(pointer.take_dropped(), 2);
    let queued = ops(&mut pointer);
    assert_eq!(queued.len(), FakePointer::CAPACITY);
    assert!(queued.iter().all(|op| matches!(op, PointerOp::Button { button: PointerButton::Left, .. })));
}

#[test]
fn a_refused_release_keeps_the_button_until_a_forced_release() {
    let mut pointer = FakePointer::new();
    let (keep, other) = (ModIndex(0), ModIndex(1));
    pointer.hold(keep, PointerButton::Middle);
    for _ in 0..(FakePointer::CAPACITY - 1) / 2 {
        pointer.hold(other, PointerButton::Left);
        pointer.release(other, PointerButton::Left);
    }
    pointer.hold(other, PointerButton::Left);

    assert!(!pointer.release(keep, PointerButton::Middle));
    assert!(pointer.is_held(PointerButton::Middle));

    assert!(pointer.release_mod(keep).contains(keep));
    assert!(!pointer.is_held(PointerButton::Middle));
    let queued = ops(&mut pointer);
    assert_eq!(queued.len(), FakePointer::CAPACITY + 1);
    assert_eq!(queued[FakePointer::CAPACITY], up(PointerButton::Middle));
}

#[test]
fn forced_releases_pass_a_full_queue_up_to_max_pending() {
    let mut pointer = FakePointer::new();
    let (m, other) = (ModIndex(0), ModIndex(1));
    for _ in 0..(FakePointer::CAPACITY - 4) / 2 {
        pointer.hold(other, PointerButton::Left);
        pointer.release(other, PointerButton::Left);
    }
    pointer.hold(m, PointerButton::Left);
    pointer.hold(m, PointerButton::Right);
    pointer.hold(m, PointerButton::Middle);
    assert!(pointer.wheel(1));
    assert_eq!(pointer.pending(), FakePointer::CAPACITY);

    assert!(pointer.release_all().contains(m));

    assert_eq!(pointer.pending(), FakePointer::MAX_PENDING);
    assert_eq!(FakePointer::MAX_PENDING, FakePointer::CAPACITY + 3);
    assert!(!pointer.is_held(PointerButton::Left));
    assert!(!pointer.is_held(PointerButton::Right));
    assert!(!pointer.is_held(PointerButton::Middle));
    pointer.take_dropped();

    assert!(!pointer.has_room());
    assert!(!pointer.hold(other, PointerButton::Left));
    assert!(!pointer.wheel(1));
    assert_eq!(pointer.pending(), FakePointer::MAX_PENDING);
    assert_eq!(pointer.take_dropped(), 2);
    let queued = ops(&mut pointer);
    assert_eq!(
        queued[FakePointer::CAPACITY..],
        [up(PointerButton::Left), up(PointerButton::Right), up(PointerButton::Middle)]
    );
}

#[test]
fn a_normal_action_after_forced_releases_brings_the_queue_back_to_capacity() {
    let mut pointer = FakePointer::new();
    let (m, other) = (ModIndex(0), ModIndex(1));
    pointer.hold(m, PointerButton::Left);
    for _ in 0..FakePointer::CAPACITY - 1 {
        pointer.wheel(1);
    }
    assert!(pointer.release_mod(m).contains(m));
    pointer.hold(other, PointerButton::Right);
    assert!(pointer.release_mod(other).contains(other));
    assert_eq!(pointer.pending(), FakePointer::CAPACITY + 1);

    assert!(pointer.wheel(2));

    assert_eq!(pointer.pending(), FakePointer::CAPACITY);
    let queued = ops(&mut pointer);
    assert_eq!(queued[0], down(PointerButton::Left));
    assert_eq!(queued[FakePointer::CAPACITY - 1], PointerOp::Wheel { steps: 2 });
}

#[test]
fn a_game_up_is_queued_for_a_free_button_and_refused_on_a_full_queue() {
    let mut pointer = FakePointer::new();
    let m = ModIndex(0);

    assert!(pointer.queue_game_up(PointerButton::Right));
    assert_eq!(ops(&mut pointer), vec![up(PointerButton::Right)]);

    for _ in 0..FakePointer::CAPACITY / 2 {
        pointer.hold(m, PointerButton::Left);
        pointer.release(m, PointerButton::Left);
    }
    assert!(!pointer.queue_game_up(PointerButton::Right));
    assert_eq!(pointer.pending(), FakePointer::CAPACITY);
}

#[test]
fn turns_on_one_axis_add_into_one_pending_value() {
    let mut pointer = FakePointer::new();

    assert!(pointer.turn(TurnAxis::Yaw, 1000));
    assert!(pointer.turn(TurnAxis::Yaw, 2500));
    assert!(pointer.turn(TurnAxis::Pitch, -300));
    assert!(pointer.turn(TurnAxis::Pitch, 100));

    assert_eq!(pointer.pending(), 2);
    assert_eq!(
        ops(&mut pointer),
        vec![
            PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees: -200 },
            PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: 3500 },
        ]
    );
    assert_eq!(pointer.pending(), 0);
    assert_eq!(ops(&mut pointer), vec![]);
}

#[test]
fn turns_that_cancel_out_queue_nothing() {
    let mut pointer = FakePointer::new();

    pointer.turn(TurnAxis::Yaw, 700);
    pointer.turn(TurnAxis::Yaw, -700);

    assert_eq!(pointer.pending(), 0);
    assert_eq!(ops(&mut pointer), vec![]);
}

#[test]
fn a_pending_turn_is_held_to_one_full_turn_either_way() {
    let limit = FakePointer::MAX_TURN_MILLIDEGREES;
    let mut pointer = FakePointer::new();

    pointer.turn(TurnAxis::Yaw, 300_000);
    pointer.turn(TurnAxis::Yaw, 300_000);
    assert_eq!(ops(&mut pointer), vec![PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: limit }]);

    pointer.turn(TurnAxis::Yaw, -300_000);
    pointer.turn(TurnAxis::Yaw, -300_000);
    assert_eq!(ops(&mut pointer), vec![PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: -limit }]);

    pointer.turn(TurnAxis::Pitch, i32::MAX);
    pointer.turn(TurnAxis::Pitch, 1000);
    assert_eq!(ops(&mut pointer), vec![PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees: limit }]);
}

#[test]
fn a_turn_past_the_bound_on_one_axis_leaves_the_other_axis_alone() {
    let mut pointer = FakePointer::new();

    pointer.turn(TurnAxis::Pitch, 1234);
    pointer.turn(TurnAxis::Yaw, 300_000);
    pointer.turn(TurnAxis::Yaw, 300_000);

    assert_eq!(
        ops(&mut pointer),
        vec![
            PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees: 1234 },
            PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: FakePointer::MAX_TURN_MILLIDEGREES },
        ]
    );
}
