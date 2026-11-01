use amm_abi::types::{AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmHandle, AmmStatus};
use amm_core::errors::LinkError;
use amm_core::structs::capture::{CaptureStreamFrame, CaptureStreamRequest};
use amm_core::traits::AbiSized;

use super::{ApiTable, LentStreamFrame};

/// A mod's view of the manager's capture stream: game-only copies of the back buffer, taken after the game has
/// drawn and before any mod layer, toast or the panel. Each `request` is filled by the next Present after it, and
/// `try_take` gives the frames back in request order with their tags. The stream has a few slots (4 in ABI 1.9);
/// a request with no free slot is `Busy` (`LinkError::Status(AmmStatus::Busy)`), and the caller takes a frame
/// before it asks again. A request needs an active `Hud` or `Overlay` lease. Each frame reports its size, row pitch
/// and DXGI format; the pixels are as the game drew them, never converted. Made by `ManagerLink::capture_stream`; it
/// names the link's registration, so after the link is dropped every call fails with `UnknownHandle`. Game thread
/// only, never from inside a frame callback. A manager older than ABI 1.9 is `Unsupported`.
///
/// A render loop takes frames with `try_take_into`, which reuses one buffer; `try_take` allocates a whole frame on
/// each call.
///
/// A change of the back buffer size while frames are in flight (a window resize, or a render size forced or ended)
/// fails each of those frames, in order, with `CaptureStreamFault::Resized`; the frames asked for after the change
/// come at the new size. A caller that forces a render size waits until `ManagerLink::render_size` reports it before
/// the first request, so a `Resized` frame then means the size changed under the stream.
///
/// `close`, or a drop, ends the stream: the frames not taken are dropped, and once the stream is empty the manager
/// frees its pixel buffers and the overlay drops its stream staging textures. Keep one `CaptureStream` for the whole
/// render; a new one starts a new stream. Unregister ends the stream too.
#[derive(Debug)]
pub struct CaptureStream {
    table: ApiTable,
    handle: AmmHandle,
}

impl CaptureStream {
    pub(crate) fn new(table: ApiTable, handle: AmmHandle) -> Self {
        Self { table, handle }
    }

    /// Asks for one frame of the next Present, tagged with `tag`. `Busy` when every slot is in use or another mod
    /// uses the stream; `NotPermitted` without an active `Hud` or `Overlay` lease.
    pub fn request(&self, tag: u64) -> Result<(), LinkError> {
        self.send(CaptureStreamRequest::new(tag))
    }

    /// Asks for one frame of the Present numbered `present_seq` only (`ManagerLink::present_count` numbers them),
    /// tagged with `tag`. The frame waits for that Present, and `try_take` reports it as
    /// `CaptureStreamFault::Missed` when that Present passed before the manager saw the request. A `present_seq`
    /// of 0 is `request`. Errors as `request`.
    pub fn request_at(&self, tag: u64, present_seq: u64) -> Result<(), LinkError> {
        self.send(CaptureStreamRequest::at(tag, present_seq))
    }

    fn send(&self, request_value: CaptureStreamRequest) -> Result<(), LinkError> {
        let Some(request) = self.table.api().capture_stream_request else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let request_value = AmmCaptureStreamRequest::from(request_value);
        // SAFETY: the request lives for the call; the handle is the link's
        match unsafe { request(self.handle, &request_value) } {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// The oldest frame once it is done, its pixels copied out of the manager, or `None` while it is in flight or
    /// no frame is asked for. A frame the manager could not copy is `LinkError::StreamFrameFailed` with its tag and
    /// fault; it takes that frame's place in the order. Allocates the frame's pixels: `try_take_into` reuses a
    /// buffer.
    pub fn try_take(&self) -> Result<Option<CaptureStreamFrame>, LinkError> {
        let mut frame = CaptureStreamFrame::default();
        Ok(self.try_take_into(&mut frame)?.then_some(frame))
    }

    /// `try_take` into `frame`: true when a frame was written to it. Its pixel buffer is reused, so a frame of the
    /// same size as the last one does not allocate. On `false` or an error `frame` is left as it was.
    pub fn try_take_into(&self, frame: &mut CaptureStreamFrame) -> Result<bool, LinkError> {
        let Some(take) = self.table.api().capture_stream_take else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let mut taken = AmmCaptureStreamFrame::blank();
        // SAFETY: the out frame lives for the call; the handle is the link's
        match unsafe { take(self.handle, &mut taken) } {
            AmmStatus::Ok => {}
            AmmStatus::Busy => return Ok(false),
            other => return Err(LinkError::from(other)),
        }
        if let Some(failure) = taken.failure() {
            return Err(LinkError::StreamFrameFailed(failure));
        }
        frame.pixels.clear();
        if !taken.pixels.is_null() {
            // SAFETY: the manager lends pixels_len readable bytes at the non-null pixels until the next take that
            // returns a frame, and no other call runs between the take and this copy
            frame.pixels.extend_from_slice(unsafe { std::slice::from_raw_parts(taken.pixels, taken.pixels_len) });
        }
        (frame.tag, frame.width, frame.height, frame.row_pitch, frame.format) =
            (taken.tag, taken.width, taken.height, taken.row_pitch, taken.format);
        frame.present_seq = taken.present_seq;
        Ok(true)
    }

    /// `try_take` without the copy: the oldest frame once it is done, its pixels read in place from the manager's
    /// buffer, which stays the mod's until `give_back` with the frame's `lend_id`. `None` while the oldest frame
    /// is in flight, no frame is asked for, or 8 frames are lent already (give one back first). A frame the
    /// manager could not copy is `LinkError::StreamFrameFailed`, as for `try_take`. A manager without the slot is
    /// `Unsupported`.
    pub fn lend(&self) -> Result<Option<LentStreamFrame>, LinkError> {
        let Some(lend) = self.table.api().capture_stream_lend else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        let mut lent = AmmCaptureStreamFrame::blank();
        // SAFETY: the out frame lives for the call; the handle is the link's
        match unsafe { lend(self.handle, &mut lent) } {
            AmmStatus::Ok => {}
            AmmStatus::Busy => return Ok(None),
            other => return Err(LinkError::from(other)),
        }
        if let Some(failure) = lent.failure() {
            return Err(LinkError::StreamFrameFailed(failure));
        }
        Ok(Some(LentStreamFrame::new(
            (lent.tag, lent.width, lent.height, lent.row_pitch, lent.format),
            (lent.present_seq, lent.lend_id),
            lent.pixels,
            lent.pixels_len,
        )))
    }

    /// Ends the lend `lend_id`: the manager reuses the buffer, and the `LentStreamFrame` of that id must be dropped
    /// first, as its pixels are gone (`LentStreamFrame::pixels` is `unsafe` for this reason). An id the manager
    /// does not hold is `InvalidArgument`. A manager without the slot is `Unsupported`.
    pub fn give_back(&self, lend_id: u64) -> Result<(), LinkError> {
        let Some(give_back) = self.table.api().capture_stream_give_back else {
            return Err(LinkError::from(AmmStatus::Unsupported));
        };
        // SAFETY: give_back takes a plain handle and id
        match unsafe { give_back(self.handle, lend_id) } {
            AmmStatus::Ok => Ok(()),
            other => Err(LinkError::from(other)),
        }
    }

    /// Ends the stream now; a drop does the same.
    pub fn close(self) {}

    /// The frames the mod may ask for now: the free slots, or 0 while another mod uses the stream, on a manager
    /// older than ABI 1.9, or after the link is dropped.
    pub fn free_slots(&self) -> usize {
        let Some(free) = self.table.api().capture_stream_free else {
            return 0;
        };
        let mut count = 0u32;
        // SAFETY: the out count lives for the call; the handle is the link's
        match unsafe { free(self.handle, &mut count) } {
            AmmStatus::Ok => count as usize,
            _ => 0,
        }
    }
}

impl Drop for CaptureStream {
    fn drop(&mut self) {
        if let Some(close) = self.table.api().capture_stream_close {
            // SAFETY: close takes a plain handle; a handle of a dropped link answers UnknownHandle
            unsafe { close(self.handle) };
        }
    }
}
