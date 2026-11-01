use amm_abi::types::{AmmHandle, AmmStatus};
use amm_core::services::IconDecoder;

use crate::services::{Boundary, HostCell};

/// Table `set_icon`.
///
/// # Safety
///
/// `png` is null or holds `png_len` readable bytes for the call.
pub unsafe extern "C" fn amm_set_icon(handle: AmmHandle, png: *const u8, png_len: usize) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives png_len readable bytes at png for the call
        let bytes = unsafe { Boundary::bytes(png, png_len) }?;
        let icon = if bytes.is_empty() {
            None
        } else {
            match IconDecoder::decode(bytes) {
                Ok(icon) => Some(icon),
                Err(error) => {
                    curia::warn!("icon refused", { slot: handle.slot, generation: handle.generation, reason: error.to_string() });
                    return Err(AmmStatus::from(&error));
                }
            }
        };
        HostCell::global().with(|runtime| runtime.set_icon(handle, icon))
    }))
}
