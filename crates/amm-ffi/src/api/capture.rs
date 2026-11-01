use std::path::Path;

use amm_abi::types::{AmmCapability, AmmHandle, AmmScreenshotLayers, AmmStatus};
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;

use crate::services::{Boundary, HostCell};

// The longest screenshot path taken, in UTF-8 bytes
const MAX_PATH_BYTES: usize = 4096;

/// Table `request_screenshot`: `amm_request_screenshot_layers` with `layers` 0, a game-only capture.
///
/// # Safety
///
/// `path` is null or holds `path_len` readable bytes, and `out_sequence` is null or writable, for the call.
pub unsafe extern "C" fn amm_request_screenshot(
    handle: AmmHandle,
    path: *const u8,
    path_len: usize,
    out_sequence: *mut u32,
) -> AmmStatus {
    let game_only = AmmScreenshotLayers::GameOnly as u8;
    // SAFETY: the caller gives the same guarantees for path and out_sequence as this function asks for
    unsafe { amm_request_screenshot_layers(handle, path, path_len, game_only, out_sequence) }
}

/// Table `request_screenshot_layers`. A null `path` or `out_sequence` is `NullPointer`, checked first; a `layers`
/// value that is not an `AmmScreenshotLayers` value, or a path longer than 4096 bytes, is then `InvalidArgument`.
///
/// # Safety
///
/// `path` is null or holds `path_len` readable bytes, and `out_sequence` is null or writable, for the call.
pub unsafe extern "C" fn amm_request_screenshot_layers(
    handle: AmmHandle,
    path: *const u8,
    path_len: usize,
    layers: u8,
    out_sequence: *mut u32,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let out_sequence = Boundary::writable(out_sequence)?;
        if path.is_null() {
            return Err(AmmStatus::NullPointer);
        }
        let layers = ScreenshotLayers::from(AmmScreenshotLayers::try_from(layers)?);
        if path_len > MAX_PATH_BYTES {
            return Err(AmmStatus::InvalidArgument);
        }
        // SAFETY: the caller gives path_len readable bytes at the non-null path for the call
        let path = Path::new(unsafe { Boundary::text(path, path_len) }?);
        let sequence = HostCell::global().with(|runtime| runtime.request_screenshot(handle, path, layers))?;
        // SAFETY: out_sequence is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_sequence.write(sequence) };
        Ok(())
    }))
}

/// Table `held_by_other`. 0 for a bad capability byte, a stale handle, no host, or a call from inside a frame
/// callback, where the host is busy.
pub extern "C" fn amm_held_by_other(handle: AmmHandle, capability: u8) -> u8 {
    let result = Boundary::guard(|| {
        let capability = Capability::from(AmmCapability::try_from(capability)?);
        HostCell::global().with(|runtime| runtime.held_by_other(handle, capability))
    });
    u8::from(result.unwrap_or(false))
}

/// Table `texture_budget`. A null `out_bytes` or `out_count` is `NullPointer`, and nothing is written.
///
/// # Safety
///
/// `out_bytes` and `out_count` are null or writable for the call.
pub unsafe extern "C" fn amm_texture_budget(handle: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
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
