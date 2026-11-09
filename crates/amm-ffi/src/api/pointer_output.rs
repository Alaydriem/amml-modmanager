use amm_abi::convert::pointer_op::PointerOpCodec;
use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `send_pointer`. An unknown `op` or a bad `value` is `InvalidArgument`, checked before the handle.
pub extern "C" fn amm_send_pointer(handle: AmmHandle, op: u32, value: i32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let op = PointerOpCodec::decode(op, value)?;
        HostCell::global().with(|runtime| runtime.send_pointer(handle, op))
    }))
}
