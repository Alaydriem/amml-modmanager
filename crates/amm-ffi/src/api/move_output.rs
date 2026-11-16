use amm_abi::convert::move_intent::MoveIntentCodec;
use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `send_move`. An axis past 1000 either way or an unknown flag is `InvalidArgument`, checked before the
/// handle.
pub extern "C" fn amm_send_move(handle: AmmHandle, forward_milli: i32, left_milli: i32, flags: u32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let intent = MoveIntentCodec::decode(forward_milli, left_milli, flags)?;
        HostCell::global().with(|runtime| runtime.send_move(handle, intent))
    }))
}
