use amm_abi::convert::pointer_op::PointerOpCodec;
use amm_abi::types::{AMM_POINTER_OP_TURN_PITCH, AMM_POINTER_OP_TURN_YAW, AmmStatus};
use amm_core::services::input::FakePointer;
use amm_core::structs::input::{PointerOp, TurnAxis};

#[test]
fn the_turn_codes_convert_both_ways() {
    let yaw = PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: -1500 };
    let pitch = PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees: 250 };

    assert_eq!(PointerOpCodec::encode(yaw), (AMM_POINTER_OP_TURN_YAW, -1500));
    assert_eq!(PointerOpCodec::encode(pitch), (AMM_POINTER_OP_TURN_PITCH, 250));
    assert_eq!(PointerOpCodec::decode(AMM_POINTER_OP_TURN_YAW, -1500), Ok(yaw));
    assert_eq!(PointerOpCodec::decode(AMM_POINTER_OP_TURN_PITCH, 250), Ok(pitch));
}

#[test]
fn a_turn_past_the_bound_does_not_decode() {
    let limit = FakePointer::MAX_TURN_MILLIDEGREES;

    assert!(PointerOpCodec::decode(AMM_POINTER_OP_TURN_YAW, limit).is_ok());
    assert_eq!(PointerOpCodec::decode(AMM_POINTER_OP_TURN_YAW, limit + 1), Err(AmmStatus::InvalidArgument));
    assert_eq!(PointerOpCodec::decode(AMM_POINTER_OP_TURN_PITCH, i32::MIN), Err(AmmStatus::InvalidArgument));
}
