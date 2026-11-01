use std::path::Path;

use amm_abi::types::{AmmCallbacks, AmmHandle, AmmStatus};

use crate::services::{Boundary, HostCell};

/// Table `register`.
///
/// # Safety
///
/// `descriptor_hcl` and `config_dir` are null or hold their lengths in readable bytes, `callbacks` is null or a
/// live `AmmCallbacks` that holds the bytes its `size` declares, and `out_handle` is null or writable, all for the
/// call. A callbacks `size` below the 2.0 struct is `InvalidArgument`.
pub unsafe extern "C" fn amm_register(
    descriptor_hcl: *const u8,
    descriptor_len: usize,
    config_dir: *const u8,
    config_dir_len: usize,
    callbacks: *const AmmCallbacks,
    out_handle: *mut AmmHandle,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        if descriptor_hcl.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        // SAFETY: the caller gives descriptor_len readable bytes at the non-null descriptor_hcl for the call
        let descriptor = unsafe { Boundary::bytes(descriptor_hcl, descriptor_len) }?;
        // SAFETY: the caller gives config_dir_len readable bytes at config_dir for the call
        let config_dir = Path::new(unsafe { Boundary::text(config_dir, config_dir_len) }?);
        // SAFETY: the caller gives a null or live callbacks struct of its declared size for the call, copied here
        let callbacks = unsafe { Boundary::sized_input(callbacks) }?;
        let out_handle = Boundary::writable(out_handle)?;
        let handle = HostCell::global().with(|runtime| runtime.register(descriptor, config_dir, callbacks))?;
        // SAFETY: out_handle is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_handle.write(handle) };
        Ok(())
    }))
}

/// Table `unregister`. It waits for a running callback of the mod, because the host lock is held around each.
/// Any status other than `Ok` leaves the registration as it was.
pub extern "C" fn amm_unregister(handle: AmmHandle) -> AmmStatus {
    let result = Boundary::guard(|| HostCell::global().with(|runtime| runtime.unregister(handle)));
    if let Err(status) = result {
        curia::debug!("unregister refused", { status: status as i32 });
    }
    Boundary::status(result)
}

/// Table `config_document`.
///
/// # Safety
///
/// `out_ptr` and `out_len` are null or writable for the call.
pub unsafe extern "C" fn amm_config_document(
    handle: AmmHandle,
    out_ptr: *mut *const u8,
    out_len: *mut usize,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
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
