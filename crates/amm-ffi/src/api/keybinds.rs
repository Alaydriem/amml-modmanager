use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Keybinds table `chord_text`. 0 for an index past the descriptor's list, a stale handle, no host or a disabled
/// manager; a buffer too small for the text gets no bytes and the text length.
///
/// # Safety
///
/// `out_buf` is null or holds `out_cap` writable bytes for the call.
pub unsafe extern "C" fn amm_chord_text(handle: AmmHandle, index: u32, out_buf: *mut u8, out_cap: usize) -> u32 {
    let result = Boundary::table(|| {
        let action = u16::try_from(index).map_err(|_| AmmStatus::InvalidArgument)?;
        let Some(text) = HostCell::global().with(|runtime| runtime.chord_text(handle, action))? else {
            return Ok(0);
        };
        let len = u32::try_from(text.len()).map_err(|_| AmmStatus::InvalidArgument)?;
        if text.len() > out_cap {
            return Ok(len);
        }
        if out_buf.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: out_buf is non-null and the caller gives out_cap writable bytes, which hold the text, for the call
        unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), out_buf, text.len()) };
        Ok(len)
    });
    result.unwrap_or(0)
}

/// Keybinds table `bind_down`. 0 for an index past the descriptor's list, a stale handle, no host, a disabled manager
/// or a call from inside a frame callback, where the host is busy.
pub extern "C" fn amm_bind_down(handle: AmmHandle, index: u32) -> u8 {
    let result = Boundary::table(|| {
        let action = u16::try_from(index).map_err(|_| AmmStatus::InvalidArgument)?;
        HostCell::global().with(|runtime| runtime.bind_down(handle, action))
    });
    u8::from(result.unwrap_or(false))
}
