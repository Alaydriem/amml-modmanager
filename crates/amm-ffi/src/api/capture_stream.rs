use amm_abi::types::{AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmHandle, AmmStatus};
use amm_core::structs::capture::CaptureStreamRequest;

use crate::services::{Boundary, HostCell};

/// Table `capture_stream_request`. A null `request` is `NullPointer`; a `size` below the 2.0 struct is
/// `InvalidArgument`.
///
/// # Safety
///
/// `request` is null or points at a live `AmmCaptureStreamRequest` that holds the bytes its `size` declares, for the
/// call.
pub unsafe extern "C" fn amm_capture_stream_request(
    handle: AmmHandle,
    request: *const AmmCaptureStreamRequest,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives a null or live request of its declared size for the call
        let request = CaptureStreamRequest::from(unsafe { Boundary::sized_input(request) }?);
        HostCell::global().with(|runtime| runtime.request_stream_frame(handle, request))
    }))
}

/// Table `capture_stream_close`.
pub extern "C" fn amm_capture_stream_close(handle: AmmHandle) -> AmmStatus {
    Boundary::status(Boundary::guard(|| HostCell::global().with(|runtime| runtime.close_stream(handle))))
}

/// Table `capture_stream_take`. A null `out_frame` is `NullPointer` and a `size` below the 2.0 struct is
/// `InvalidArgument`, and nothing is written; `Busy` writes nothing. The frame is written up to the `size` the caller
/// declares, which then holds the bytes written.
///
/// # Safety
///
/// `out_frame` is null or points at a frame that holds the bytes its `size` declares, writable for the call.
pub unsafe extern "C" fn amm_capture_stream_take(
    handle: AmmHandle,
    out_frame: *mut AmmCaptureStreamFrame,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives a null pointer or a frame whose size field is readable; a frame the mod cannot
        // take is refused before the stream gives it up
        unsafe { Boundary::sized_len(out_frame.cast_const()) }?;
        let frame = HostCell::global().with(|runtime| runtime.take_stream_frame(handle))?;
        // SAFETY: the caller gives a frame of its declared size, writable for the call
        unsafe { Boundary::sized_output(out_frame, &frame) }
    }))
}

/// Table `capture_stream_lend`. As `amm_capture_stream_take`, with the pixels lent until the frame's `lend_id` is
/// given back.
///
/// # Safety
///
/// `out_frame` is null or points at a frame that holds the bytes its `size` declares, writable for the call.
pub unsafe extern "C" fn amm_capture_stream_lend(
    handle: AmmHandle,
    out_frame: *mut AmmCaptureStreamFrame,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives a null pointer or a frame whose size field is readable; a frame the mod cannot
        // take is refused before the stream gives it up
        unsafe { Boundary::sized_len(out_frame.cast_const()) }?;
        let frame = HostCell::global().with(|runtime| runtime.lend_stream_frame(handle))?;
        // SAFETY: the caller gives a frame of its declared size, writable for the call
        unsafe { Boundary::sized_output(out_frame, &frame) }
    }))
}

/// Table `capture_stream_give_back`. An id the mod does not hold is `InvalidArgument`.
pub extern "C" fn amm_capture_stream_give_back(handle: AmmHandle, lend_id: u64) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        HostCell::global().with(|runtime| runtime.give_back_stream_frame(handle, lend_id))
    }))
}

/// Table `present_count`. A null `out_count` is `NullPointer`, and nothing is written.
///
/// # Safety
///
/// `out_count` is null or writable for the call.
pub unsafe extern "C" fn amm_present_count(handle: AmmHandle, out_count: *mut u64) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let out_count = Boundary::writable(out_count)?;
        let count = HostCell::global().with(|runtime| runtime.present_count(handle))?;
        // SAFETY: out_count is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_count.write(count) };
        Ok(())
    }))
}

/// Table `capture_stream_free`. A null `out_free` is `NullPointer`, and nothing is written.
///
/// # Safety
///
/// `out_free` is null or writable for the call.
pub unsafe extern "C" fn amm_capture_stream_free(handle: AmmHandle, out_free: *mut u32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let out_free = Boundary::writable(out_free)?;
        let free = HostCell::global().with(|runtime| runtime.stream_free_slots(handle))?;
        // SAFETY: out_free is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out_free.write(free) };
        Ok(())
    }))
}
