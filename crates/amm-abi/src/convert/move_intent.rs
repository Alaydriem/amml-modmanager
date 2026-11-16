use amm_core::structs::input::MoveIntent;

use crate::types::{AMM_MOVE_AXIS_MAX, AMM_MOVE_JUMP, AMM_MOVE_SNEAK, AMM_MOVE_SPRINT, AmmStatus};

/// The `forward_milli`, `left_milli` and `flags` of `send_move` for a `MoveIntent`, both ways. The only place that
/// knows them.
pub struct MoveIntentCodec;

impl MoveIntentCodec {
    const ALL_FLAGS: u32 = AMM_MOVE_JUMP | AMM_MOVE_SNEAK | AMM_MOVE_SPRINT;
    const MILLI: f32 = 1000.0;

    /// The values a mod passes to `send_move` for `intent`. An axis outside -1.0 to 1.0 encodes past
    /// `AMM_MOVE_AXIS_MAX`, so the manager refuses it rather than moving the player at a value the mod did not ask
    /// for.
    pub fn encode(intent: MoveIntent) -> (i32, i32, u32) {
        let flags = Self::flag(intent.jump, AMM_MOVE_JUMP)
            | Self::flag(intent.sneak, AMM_MOVE_SNEAK)
            | Self::flag(intent.sprint, AMM_MOVE_SPRINT);
        (Self::milli(intent.forward), Self::milli(intent.left), flags)
    }

    /// The intent for the values of `send_move`. An axis past `AMM_MOVE_AXIS_MAX` either way, or a flag bit other
    /// than jump, sneak and sprint, is `InvalidArgument`.
    pub fn decode(forward_milli: i32, left_milli: i32, flags: u32) -> Result<MoveIntent, AmmStatus> {
        let in_range = |value: i32| value.unsigned_abs() <= AMM_MOVE_AXIS_MAX.unsigned_abs();
        if !in_range(forward_milli) || !in_range(left_milli) || flags & !Self::ALL_FLAGS != 0 {
            return Err(AmmStatus::InvalidArgument);
        }
        Ok(MoveIntent {
            forward: forward_milli as f32 / Self::MILLI,
            left: left_milli as f32 / Self::MILLI,
            jump: flags & AMM_MOVE_JUMP != 0,
            sneak: flags & AMM_MOVE_SNEAK != 0,
            sprint: flags & AMM_MOVE_SPRINT != 0,
        })
    }

    fn milli(axis: f32) -> i32 {
        if axis.is_nan() {
            return i32::MAX;
        }
        (axis * Self::MILLI).round() as i32
    }

    fn flag(on: bool, bit: u32) -> u32 {
        if on { bit } else { 0 }
    }
}
