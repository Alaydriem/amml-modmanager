use amm_abi::types::{AmmCallbacks, AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Layers table `set_callbacks`. A null `callbacks` is `NullPointer` and a `size` below the 3.0 struct is
/// `InvalidArgument`; the callbacks the mod had stay.
///
/// # Safety
///
/// `callbacks` is null or a live `AmmCallbacks` that holds the bytes its `size` declares, for the call.
pub unsafe extern "C" fn amm_set_callbacks(handle: AmmHandle, callbacks: *const AmmCallbacks) -> AmmStatus {
    Boundary::status(Boundary::table(|| {
        // SAFETY: the caller gives a null or live callbacks struct of its declared size for the call, copied here
        let callbacks = unsafe { Boundary::sized_input(callbacks) }?;
        HostCell::global().with(|runtime| runtime.set_callbacks(handle, callbacks))
    }))
}

/// Layers table `texture_budget`. A null `out_bytes` or `out_count` is `NullPointer`, and nothing is written.
///
/// # Safety
///
/// `out_bytes` and `out_count` are null or writable for the call.
pub unsafe extern "C" fn amm_texture_budget(handle: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus {
    Boundary::status(Boundary::table(|| {
        let (out_bytes, out_count) = (Boundary::writable(out_bytes)?, Boundary::writable(out_count)?);
        let grant = HostCell::global().with(|runtime| runtime.texture_budget(handle))?;
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the
        // caller gives
        unsafe {
            out_bytes.write(grant.bytes);
            out_count.write(grant.count);
        }
        Ok(())
    }))
}

/// Layers table `set_cursor_mode`. A value other than 0 or 1, a stale handle, a refusal or a disabled manager is
/// ignored.
pub extern "C" fn amm_set_cursor_mode(handle: AmmHandle, on: u8) {
    let result = Boundary::table(|| {
        let on = match on {
            0 => false,
            1 => true,
            _ => return Err(AmmStatus::InvalidArgument),
        };
        HostCell::global().with(|runtime| runtime.set_cursor_mode(handle, on))
    });
    if let Err(status) = result {
        curia::debug!("set_cursor_mode ignored", { status: status as i32 });
    }
}
