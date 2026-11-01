use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `set_setting`.
///
/// # Safety
///
/// `path` and `value_hcl` are null or hold their lengths in readable bytes for the call.
pub unsafe extern "C" fn amm_set_setting(
    handle: AmmHandle,
    path: *const u8,
    path_len: usize,
    value_hcl: *const u8,
    value_len: usize,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives path_len readable bytes at path for the call
        let path = unsafe { Boundary::text(path, path_len) }?;
        // SAFETY: the caller gives value_len readable bytes at value_hcl for the call
        let value = unsafe { Boundary::text(value_hcl, value_len) }?;
        HostCell::global().with(|runtime| runtime.set_setting(handle, path, value))
    }))
}
