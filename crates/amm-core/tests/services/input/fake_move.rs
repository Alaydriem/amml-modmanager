use amm_core::services::input::FakeMove;
use amm_core::structs::input::MoveIntent;
use amm_core::structs::registry::ModIndex;

const A: ModIndex = ModIndex(0);
const B: ModIndex = ModIndex(1);

fn forward() -> MoveIntent {
    MoveIntent { forward: 1.0, ..MoveIntent::ZERO }
}

#[test]
fn one_intent_is_held_and_a_second_mod_replaces_the_first() {
    let mut moves = FakeMove::new();

    assert_eq!(moves.set(A, forward()), None);
    let replaced = moves.set(B, MoveIntent { left: 1.0, ..MoveIntent::ZERO });

    assert_eq!(replaced, Some(A));
    assert_eq!(moves.owner(), Some(B));
    assert_eq!(moves.current(), MoveIntent { left: 1.0, ..MoveIntent::ZERO });
}

#[test]
fn a_zero_intent_clears_and_holds_nothing() {
    let mut moves = FakeMove::new();
    moves.set(A, forward());

    moves.set(A, MoveIntent::ZERO);

    assert_eq!(moves.owner(), None);
    assert_eq!(moves.current(), MoveIntent::ZERO);
}

#[test]
fn clearing_names_the_owner_once() {
    let mut moves = FakeMove::new();
    moves.set(A, forward());

    assert!(moves.clear_mod(B).is_empty());
    assert!(moves.clear_except(A).is_empty());
    assert!(moves.clear_mod(A).contains(A));
    assert!(moves.clear_all().is_empty());

    moves.set(B, forward());
    assert!(moves.clear_except(A).contains(B));
    assert_eq!(moves.current(), MoveIntent::ZERO);
}
