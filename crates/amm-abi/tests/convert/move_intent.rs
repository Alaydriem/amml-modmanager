use amm_abi::convert::move_intent::MoveIntentCodec;
use amm_abi::types::{AMM_MOVE_JUMP, AMM_MOVE_SNEAK, AMM_MOVE_SPRINT, AmmStatus};
use amm_core::structs::input::MoveIntent;

#[test]
fn the_axes_cross_as_milli_units_and_the_actions_as_flags() {
    let intent = MoveIntent { forward: 1.0, left: -0.5, jump: true, sneak: false, sprint: true };

    assert_eq!(MoveIntentCodec::encode(intent), (1000, -500, AMM_MOVE_JUMP | AMM_MOVE_SPRINT));
    assert_eq!(MoveIntentCodec::decode(1000, -500, AMM_MOVE_JUMP | AMM_MOVE_SPRINT), Ok(intent));
    assert_eq!(MoveIntentCodec::decode(0, 0, AMM_MOVE_SNEAK), Ok(MoveIntent { sneak: true, ..MoveIntent::ZERO }));
}

#[test]
fn an_axis_past_1000_or_an_unknown_flag_does_not_decode() {
    assert!(MoveIntentCodec::decode(-1000, 1000, 0).is_ok());
    assert_eq!(MoveIntentCodec::decode(1001, 0, 0), Err(AmmStatus::InvalidArgument));
    assert_eq!(MoveIntentCodec::decode(0, i32::MIN, 0), Err(AmmStatus::InvalidArgument));
    assert_eq!(MoveIntentCodec::decode(0, 0, 8), Err(AmmStatus::InvalidArgument));
}

#[test]
fn an_axis_out_of_range_encodes_past_the_bound_so_the_manager_refuses_it() {
    let (forward, left, _) = MoveIntentCodec::encode(MoveIntent { forward: 2.0, left: f32::NAN, ..MoveIntent::ZERO });

    assert!(MoveIntentCodec::decode(forward, 0, 0).is_err());
    assert!(MoveIntentCodec::decode(0, left, 0).is_err());
}
