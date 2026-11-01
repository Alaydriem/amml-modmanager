use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `locale`.
///
/// # Safety
///
/// `out_buf` is null or holds `out_cap` writable bytes for the call.
pub unsafe extern "C" fn amm_locale(handle: AmmHandle, out_buf: *mut u8, out_cap: usize) -> u32 {
    let result = Boundary::guard(|| {
        let code = HostCell::global().with(|runtime| runtime.read_locale(handle))?;
        let len = u32::try_from(code.len()).map_err(|_| AmmStatus::InvalidArgument)?;
        if code.len() > out_cap {
            return Ok(len);
        }
        if code.is_empty() {
            return Ok(0);
        }
        if out_buf.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: out_buf is non-null and the caller gives out_cap writable bytes, which hold the code, for the call
        unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), out_buf, code.len()) };
        Ok(len)
    });
    result.unwrap_or(0)
}
