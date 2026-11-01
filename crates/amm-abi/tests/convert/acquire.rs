use amm_abi::types::{AmmAcquireOutcome, AmmAcquireResult};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;

#[test]
fn every_outcome_survives_core_to_abi_to_core() {
    for outcome in [
        AcquireOutcome::Granted,
        AcquireOutcome::Preempted,
        AcquireOutcome::Denied { holder: ModIndex(0) },
        AcquireOutcome::Denied { holder: ModIndex(15) },
    ] {
        assert_eq!(AcquireOutcome::from(AmmAcquireResult::from(outcome)), outcome);
    }
}

#[test]
fn denied_carries_the_holder_and_the_others_carry_no_holder() {
    let denied = AmmAcquireResult::from(AcquireOutcome::Denied { holder: ModIndex(3) });
    assert_eq!(denied.outcome, AmmAcquireOutcome::Denied);
    assert_eq!(denied.holder, 3);
    assert_eq!(AmmAcquireResult::from(AcquireOutcome::Granted).holder, AmmAcquireResult::NO_HOLDER);
    assert_eq!(AmmAcquireResult::from(AcquireOutcome::Preempted).holder, AmmAcquireResult::NO_HOLDER);
}
