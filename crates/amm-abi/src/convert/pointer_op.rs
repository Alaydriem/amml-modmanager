use amm_core::services::input::FakePointer;
use amm_core::structs::input::{PointerButton, PointerOp, TurnAxis};

use crate::types::{
    AMM_POINTER_OP_LEFT, AMM_POINTER_OP_MIDDLE, AMM_POINTER_OP_RIGHT, AMM_POINTER_OP_TURN_PITCH,
    AMM_POINTER_OP_TURN_YAW, AMM_POINTER_OP_WHEEL, AmmStatus,
};

/// The `op` and `value` numbers of `send_pointer` for a `PointerOp`, both ways. The only place that knows them.
pub struct PointerOpCodec;

impl PointerOpCodec {
    /// The `(op, value)` a mod passes to `send_pointer` for `op`.
    pub fn encode(op: PointerOp) -> (u32, i32) {
        match op {
            PointerOp::Button { button, down } => {
                let code = match button {
                    PointerButton::Left => AMM_POINTER_OP_LEFT,
                    PointerButton::Right => AMM_POINTER_OP_RIGHT,
                    PointerButton::Middle => AMM_POINTER_OP_MIDDLE,
                };
                (code, i32::from(down))
            }
            PointerOp::Wheel { steps } => (AMM_POINTER_OP_WHEEL, steps),
            PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees } => (AMM_POINTER_OP_TURN_YAW, millidegrees),
            PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees } => (AMM_POINTER_OP_TURN_PITCH, millidegrees),
        }
    }

    /// The action for an `(op, value)` of `send_pointer`. An unknown `op`, a button `value` other than 0 or 1, or a
    /// wheel `value` past `FakePointer::MAX_WHEEL_STEPS` either way, or a turn `value` past
    /// `FakePointer::MAX_TURN_MILLIDEGREES` either way, is `InvalidArgument`.
    pub fn decode(op: u32, value: i32) -> Result<PointerOp, AmmStatus> {
        let button = match op {
            AMM_POINTER_OP_LEFT => PointerButton::Left,
            AMM_POINTER_OP_RIGHT => PointerButton::Right,
            AMM_POINTER_OP_MIDDLE => PointerButton::Middle,
            AMM_POINTER_OP_WHEEL if value.unsigned_abs() <= FakePointer::MAX_WHEEL_STEPS.unsigned_abs() => {
                return Ok(PointerOp::Wheel { steps: value });
            }
            AMM_POINTER_OP_TURN_YAW | AMM_POINTER_OP_TURN_PITCH
                if value.unsigned_abs() <= FakePointer::MAX_TURN_MILLIDEGREES.unsigned_abs() =>
            {
                let axis = if op == AMM_POINTER_OP_TURN_YAW { TurnAxis::Yaw } else { TurnAxis::Pitch };
                return Ok(PointerOp::Turn { axis, millidegrees: value });
            }
            _ => return Err(AmmStatus::InvalidArgument),
        };
        match value {
            0 => Ok(PointerOp::Button { button, down: false }),
            1 => Ok(PointerOp::Button { button, down: true }),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
