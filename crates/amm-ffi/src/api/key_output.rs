use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `watch_keys`. A null `vks` with a `count` above 0 is `NullPointer`.
///
/// # Safety
///
/// `vks` is null or points at `count` readable `u32` for the call.
pub unsafe extern "C" fn amm_watch_keys(handle: AmmHandle, vks: *const u32, count: usize) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: vks is null or readable for count values for the call, as the caller gives
        let vks = unsafe { Boundary::values(vks, count) }?;
        HostCell::global().with(|runtime| runtime.watch_keys(handle, vks))
    }))
}

/// Table `send_key`. A `down` other than 0 or 1 is `InvalidArgument`.
pub extern "C" fn amm_send_key(handle: AmmHandle, vk: u32, down: u8) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        if down > 1 {
            return Err(AmmStatus::InvalidArgument);
        }
        HostCell::global().with(|runtime| runtime.send_key(handle, vk, down == 1))
    }))
}
