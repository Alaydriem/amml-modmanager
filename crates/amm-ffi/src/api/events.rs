use amm_abi::types::{AmmEvent, AmmHandle};

use crate::services::{Boundary, HostCell};

/// Table `poll_events`. A stale handle or a null buffer drains nothing.
///
/// # Safety
///
/// `out_buf` is null or has room for `capacity` events, writable for the call.
pub unsafe extern "C" fn amm_poll_events(handle: AmmHandle, out_buf: *mut AmmEvent, capacity: usize) -> usize {
    let result = Boundary::guard(|| {
        if capacity == 0 {
            return Ok(0);
        }
        let out_buf = Boundary::writable(out_buf)?;
        HostCell::global().with(|runtime| {
            runtime.poll_events(handle, capacity, |position, event| {
                // SAFETY: the caller gives room for capacity events at out_buf, and position is below capacity
                unsafe { out_buf.add(position).write(event) }
            })
        })
    });
    result.unwrap_or(0)
}
