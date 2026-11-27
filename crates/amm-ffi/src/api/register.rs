use std::path::Path;

use amm_abi::types::{AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Core table `register`.
///
/// # Safety
///
/// `descriptor` and `config_dir` are null or hold their lengths in readable bytes, and `out_handle` is null or
/// writable, all for the call.
pub unsafe extern "C" fn amm_register(
    descriptor: *const u8,
    descriptor_len: usize,
    config_dir: *const u8,
    config_dir_len: usize,
    out_handle: *mut AmmHandle,
) -> AmmStatus {
    Boundary::status(Boundary::table(|| {
        if descriptor.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: the caller gives descriptor_len readable bytes at the non-null descriptor for the call
        let descriptor = unsafe { Boundary::bytes(descriptor, descriptor_len) }?;
        // SAFETY: the caller gives config_dir_len readable bytes at config_dir for the call
        let config_dir = Path::new(unsafe { Boundary::text(config_dir, config_dir_len) }?);
        let out_handle = Boundary::writable(out_handle)?;
        let handle = HostCell::global().with(|runtime| runtime.register(descriptor, config_dir))?;
        // SAFETY: out_handle is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_handle.write(handle) };
        Ok(())
    }))
}

/// Core table `unregister`. It waits for a running Present, whose frame callbacks may still run.
/// Any status other than `Ok` leaves the registration as it was.
pub extern "C" fn amm_unregister(handle: AmmHandle) -> AmmStatus {
    let result = Boundary::table(|| HostCell::global().with_callbacks_held(|runtime| runtime.unregister(handle)));
    if let Err(status) = result {
        curia::debug!("unregister refused", { status: status as i32 });
    }
    Boundary::status(result)
}
