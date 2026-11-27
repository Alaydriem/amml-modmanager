use amm_abi::types::{AmmAcquireOutcome, AmmAcquireResult, AmmCapability, AmmHandle, AmmStatus};
use amm_core::structs::descriptor::Capability;

use crate::services::{Boundary, HostCell};

const DENIED: AmmAcquireResult =
    AmmAcquireResult { outcome: AmmAcquireOutcome::Denied, holder: AmmAcquireResult::NO_HOLDER };

/// Leases table `acquire`. A bad capability byte (the unused 1 among them), a stale handle, a disabled manager or any
/// refusal is denied with no holder.
pub extern "C" fn amm_acquire(handle: AmmHandle, capability: u8) -> AmmAcquireResult {
    let result = Boundary::table(|| {
        let capability = Capability::from(AmmCapability::try_from(capability)?);
        HostCell::global().with(|runtime| runtime.acquire(handle, capability))
    });
    result.map_or(DENIED, AmmAcquireResult::from)
}

/// Leases table `release`. A bad capability byte, a stale handle or a disabled manager is ignored.
pub extern "C" fn amm_release(handle: AmmHandle, capability: u8) {
    let result = Boundary::table(|| {
        let capability = Capability::from(AmmCapability::try_from(capability)?);
        HostCell::global().with(|runtime| runtime.release(handle, capability))
    });
    if let Err(status) = result {
        curia::debug!("release ignored", { status: status as i32 });
    }
}

/// Leases table `holder`. It reads the holder book without the host lock and copies the id into the caller's buffer
/// while it holds the book, so no pointer into the book leaves the call. A bad capability byte, no holder, a disabled
/// manager or a null buffer with room for the id gives 0; a buffer too small for the id gets no bytes and the id length.
///
/// # Safety
///
/// `out_buf` is null or writable for `out_cap` bytes for the call.
pub unsafe extern "C" fn amm_holder(capability: u8, out_buf: *mut u8, out_cap: usize) -> u32 {
    let result = Boundary::table(|| {
        let capability = Capability::from(AmmCapability::try_from(capability)?);
        let shared = HostCell::global().shared().ok_or(AmmStatus::ManagerMissing)?;
        let book = shared.holders().load();
        let Some(id) = book.id(capability) else {
            return Ok(0);
        };
        let len = u32::try_from(id.len()).map_err(|_| AmmStatus::InvalidArgument)?;
        if id.len() > out_cap {
            return Ok(len);
        }
        if out_buf.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: out_buf is non-null and the caller gives out_cap writable bytes, which hold the id, for the call;
        // the id lives in the book loaded above, which this call keeps alive until the copy ends
        unsafe { std::ptr::copy_nonoverlapping(id.as_ptr(), out_buf, id.len()) };
        Ok(len)
    });
    result.unwrap_or(0)
}

/// Leases table `held_by_other`. 0 for a bad capability byte, a stale handle, no host, a disabled manager or a call
/// from inside a frame callback, where the host is busy.
pub extern "C" fn amm_held_by_other(handle: AmmHandle, capability: u8) -> u8 {
    let result = Boundary::table(|| {
        let capability = Capability::from(AmmCapability::try_from(capability)?);
        HostCell::global().with(|runtime| runtime.held_by_other(handle, capability))
    });
    u8::from(result.unwrap_or(false))
}
