use amm_abi::types::AmmStatus;

use crate::services::{Boundary, HostCell};

/// Render thread, inside Present. The buffer the manager owns for the pixels of capture stream copy `id`, offered
/// in `AmmHostFrame::streams`: `height` rows of `*out_row_pitch` bytes, of which the first `width * 4` hold a row of
/// pixels in the back buffer format. The rules are those of `amm_host_capture_buffer`: the pointer is valid until
/// the first of `amm_host_stream_done`, `amm_host_textures_lost` or the next `amm_host_frame`; the overlay asks for
/// the buffer, fills every row in full and reports done within one Present, and a buffer asked for and not
/// reported by the next `amm_host_frame` fails the copy. A second call with the same size in the same Present
/// returns the same buffer. Null, with nothing written, for an id not offered or already reported, a width or
/// height of 0 or above 16384, another size than the first call, a null `out_row_pitch`, no host, or a buffer
/// that cannot be allocated (the copy then fails).
///
/// # Safety
///
/// `out_row_pitch` is null or writable for the call.
pub unsafe extern "C" fn amm_host_stream_buffer(id: u32, width: u32, height: u32, out_row_pitch: *mut u32) -> *mut u8 {
    let result = Boundary::guard(|| {
        let out_row_pitch = Boundary::writable(out_row_pitch)?;
        let (buffer, row_pitch) = HostCell::global().with(|runtime| runtime.stream_buffer(id, width, height))?;
        // SAFETY: out_row_pitch is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_row_pitch.write(row_pitch) };
        Ok(buffer)
    });
    result.unwrap_or(std::ptr::null_mut())
}

/// Render thread, inside Present. The overlay copied capture stream copy `id` into its buffer (`ok` 1, only once
/// every row of the buffer is written in full) in the DXGI format `format`, or could not (`ok` 0). The mod takes
/// the frame with `capture_stream_take`. A format other than `R8G8B8A8_UNORM(_SRGB)`, `B8G8R8A8_UNORM(_SRGB)` or
/// `R10G10B10A2_UNORM` fails the frame with `UnsupportedFormat`; the pixels are never converted.
/// `InvalidArgument` for an id not offered or already reported, or an `ok` other than 0 or 1.
pub extern "C" fn amm_host_stream_done(id: u32, ok: u8, format: u32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let ok = match ok {
            0 => false,
            1 => true,
            _ => return Err(AmmStatus::InvalidArgument),
        };
        HostCell::global().with(|runtime| runtime.stream_done(id, ok, format))
    }))
}
