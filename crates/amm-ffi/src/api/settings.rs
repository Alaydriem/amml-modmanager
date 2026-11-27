use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Settings table `config_document`.
///
/// # Safety
///
/// `out_ptr` and `out_len` are null or writable for the call.
pub unsafe extern "C" fn amm_config_document(
    handle: AmmHandle,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> AmmStatus {
    Boundary::status(Boundary::table(|| {
        let (out_ptr, out_len) = (Boundary::writable(out_ptr)?, Boundary::writable(out_len)?);
        let (text, len) = HostCell::global().with(|runtime| runtime.config_document(handle))?;
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the
        // caller gives
        unsafe {
            out_ptr.write(text);
            out_len.write(len);
        }
        Ok(())
    }))
}

/// Settings table `set_setting`.
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
    Boundary::status(Boundary::table(|| {
        // SAFETY: the caller gives path_len readable bytes at path for the call
        let path = unsafe { Boundary::text(path, path_len) }?;
        // SAFETY: the caller gives value_len readable bytes at value_hcl for the call
        let value = unsafe { Boundary::text(value_hcl, value_len) }?;
        HostCell::global().with(|runtime| runtime.set_setting(handle, path, value))
    }))
}
