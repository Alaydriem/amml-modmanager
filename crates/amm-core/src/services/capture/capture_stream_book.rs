use crate::errors::CaptureError;
use crate::services::capture::CaptureBook;
use crate::services::lease::LeaseArbiter;
use crate::structs::capture::{
    CaptureStreamEntry, CaptureStreamFailure, CaptureStreamFault, CaptureStreamFrame, CaptureStreamRequest,
    CaptureStreamStage, LentStreamFrame, PixelFormat,
};
use crate::structs::registry::ModIndex;
use crate::structs::render::RenderSize;

/// The capture stream: game-only copies of the back buffer, one per request, each filled by the next Present
/// after the request and given back to the mod in request order with its tag. It is separate from the screenshot
/// captures of `CaptureBook`: its own ring, its own overlay staging textures and its own ids, so a stream never
/// takes a screenshot's slot.
///
/// Bounds: `RING` slots, so at most `RING` frames are requested, in flight or waiting to be taken; a request past
/// it is `Busy` and the caller waits for a take. Each slot keeps one pixel buffer, and the book keeps one more,
/// the frame last lent to the mod, so at most `RING + 1` buffers of a back buffer's size are held. A frame of the
/// same size as the last one in its slot does not allocate. `close`, and an unregister, free every buffer once no
/// slot is in use, and tell the overlay (`take_release`) to drop its stream staging textures, so a finished stream
/// keeps no memory.
///
/// `lend` is `take` without the next-take limit on the pixels: the slot's buffer leaves the slot and stays with the
/// mod until `give_back`, so at most `MAX_LENT` more buffers are out. A given-back buffer waits in a spare list
/// (at most `MAX_LENT` long) and a slot takes it before it allocates. `close` keeps lent buffers until they are
/// given back and frees the spare ones; an unregister frees every one.
///
/// A change of the back buffer size fails each copy in flight with `Resized`, in order; the frames asked for after
/// it come at the new size.
///
/// The stream belongs to one mod at a time: while a slot holds a frame of one mod, a request of another mod is
/// `Busy`. A request needs an active `Hud` or `Overlay` lease, as a screenshot does. A frame whose mod loses both
/// before a Present offers it fails `NotPermitted`; one whose mod closes the stream or unregisters is dropped.
pub struct CaptureStreamBook {
    entries: [CaptureStreamEntry; Self::RING],
    // the frame last taken; its pixels are lent to the mod until the next take that returns a frame
    lent: CaptureStreamFrame,
    // the frames lent with `lend` and not given back, at most `MAX_LENT`
    lent_out: Vec<LentStreamFrame>,
    // given-back buffers a slot takes before it allocates, at most `MAX_LENT`
    spare: Vec<Vec<u8>>,
    last_lend: u64,
    last_id: u32,
    last_order: u64,
    // the buffers were freed and the overlay has not been told yet to drop its stream staging textures
    release_due: bool,
}

impl CaptureStreamBook {
    /// The ring size. Four slots let a mod ask for a frame on every Present while the GPU runs up to three frames
    /// behind the CPU (the DXGI default frame latency), so the non-blocking reads keep up without a stall.
    pub const RING: usize = 4;
    /// The Presents a copy may take from its offer to the overlay's report, as for a screenshot.
    pub const MAX_FRAMES: u32 = 30;
    /// The frames a mod may hold lent at once; a `lend` past it is refused until one is given back.
    pub const MAX_LENT: usize = 8;

    pub fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| CaptureStreamEntry::default()),
            lent: CaptureStreamFrame::default(),
            lent_out: Vec::with_capacity(Self::MAX_LENT),
            spare: Vec::with_capacity(Self::MAX_LENT),
            last_lend: 0,
            last_id: 0,
            last_order: 0,
            release_due: false,
        }
    }

    /// Books a frame for the mod in `index`, filled by the next Present. `NotPermitted` without an active `Hud` or
    /// `Overlay` lease; `Busy` when every slot is in use or the stream holds a frame of another mod. The caller
    /// checks that the mod is registered.
    pub fn request(
        &mut self,
        arbiter: &LeaseArbiter,
        index: ModIndex,
        request: CaptureStreamRequest,
    ) -> Result<(), CaptureError> {
        if !CaptureBook::may_capture(arbiter, index) {
            return Err(CaptureError::NotPermitted);
        }
        if self.held_by_other(index) {
            return Err(CaptureError::Busy);
        }
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.stage == CaptureStreamStage::Free) else {
            return Err(CaptureError::Busy);
        };
        self.last_id = self.last_id.wrapping_add(1).max(1);
        self.last_order += 1;
        entry.id = self.last_id;
        entry.owner = Some(index);
        entry.order = self.last_order;
        entry.stage = CaptureStreamStage::Pending;
        entry.frame.tag = request.tag;
        entry.target = request.present_seq;
        Ok(())
    }

    /// The slots the mod in `index` may request now: the free slots, or 0 while the stream holds a frame of
    /// another mod.
    pub fn free_slots(&self, index: ModIndex) -> usize {
        if self.held_by_other(index) {
            return 0;
        }
        self.entries.iter().filter(|entry| entry.stage == CaptureStreamStage::Free).count()
    }

    /// One Present with a `back_buffer` sized back buffer, before `offer`: a copy in flight or lost that was offered
    /// at another size fails `Resized`; a lost copy at the same size fails `DeviceLost`; a copy whose buffer the overlay
    /// took and did not report in the Present that asked for it fails `CopyFailed`, and one not reported within
    /// `MAX_FRAMES` Presents fails `TimedOut`. It does not allocate.
    pub fn advance(&mut self, back_buffer: RenderSize) {
        for entry in self.entries.iter_mut() {
            let in_flight = matches!(entry.stage, CaptureStreamStage::Offered { .. } | CaptureStreamStage::Lost);
            if in_flight && entry.size != back_buffer {
                curia::warn!("stream frame failed, the back buffer changed size", {
                    id: entry.id,
                    width: back_buffer.width,
                    height: back_buffer.height,
                });
                entry.stage = CaptureStreamStage::Failed(CaptureStreamFault::Resized);
            }
            if entry.stage == CaptureStreamStage::Lost {
                entry.stage = CaptureStreamStage::Failed(CaptureStreamFault::DeviceLost);
            }
            if let CaptureStreamStage::Offered { age, asked } = entry.stage {
                let age = age + 1;
                entry.stage = if asked {
                    curia::warn!("stream buffer not reported in the Present that asked for it", { id: entry.id });
                    CaptureStreamStage::Failed(CaptureStreamFault::CopyFailed)
                } else if age >= Self::MAX_FRAMES {
                    curia::warn!("stream frame not copied in time", { id: entry.id, frames: age });
                    CaptureStreamStage::Failed(CaptureStreamFault::TimedOut)
                } else {
                    CaptureStreamStage::Offered { age, asked }
                };
            }
        }
    }

    /// Hands the pending frames to the overlay for this Present of a `back_buffer` sized back buffer: writes their ids
    /// to `out` in request order and returns how many. A pending frame whose mod has lost its `Hud` and `Overlay`
    /// leases fails `NotPermitted` and is not offered. It does not allocate.
    pub fn offer(&mut self, arbiter: &LeaseArbiter, back_buffer: RenderSize, out: &mut [u32]) -> usize {
        self.offer_at(arbiter, back_buffer, 0, out)
    }

    /// `offer` for the Present numbered `present_seq`: each frame offered keeps that number, and `take` gives it back.
    /// A frame that names no Present, or this one, is offered; one that names a later Present stays pending, and
    /// one that names an earlier Present fails `Missed` in its place in the order.
    pub fn offer_at(
        &mut self,
        arbiter: &LeaseArbiter,
        back_buffer: RenderSize,
        present_seq: u64,
        out: &mut [u32],
    ) -> usize {
        let mut count = 0;
        while count < out.len() {
            let next = self
                .entries
                .iter_mut()
                .filter(|entry| entry.stage == CaptureStreamStage::Pending)
                .filter(|entry| entry.target <= present_seq)
                .min_by_key(|entry| entry.order);
            let Some(entry) = next else {
                break;
            };
            if !entry.owner.is_some_and(|owner| CaptureBook::may_capture(arbiter, owner)) {
                entry.stage = CaptureStreamStage::Failed(CaptureStreamFault::NotPermitted);
                continue;
            }
            if entry.target != 0 && entry.target < present_seq {
                curia::warn!("stream frame missed its Present", {
                    id: entry.id,
                    target: entry.target,
                    present: present_seq,
                });
                entry.stage = CaptureStreamStage::Failed(CaptureStreamFault::Missed);
                continue;
            }
            entry.stage = CaptureStreamStage::Offered { age: 0, asked: false };
            entry.size = back_buffer;
            entry.frame.present_seq = present_seq;
            out[count] = entry.id;
            count += 1;
        }
        count
    }

    /// The buffer the overlay copies the pixels of copy `id` into, and its row pitch: `height` rows of `width * 4`
    /// bytes. The first call takes it for the copy; a later call with the same size returns the same buffer.
    /// `None` for an id not offered or already reported, a side of 0 or past `RenderSize::MAX_SIDE`, or another
    /// size than the first call. A buffer that cannot be allocated gives `None` and fails the frame.
    pub fn buffer(&mut self, id: u32, width: u32, height: u32) -> Option<(&mut [u8], u32)> {
        let valid = RenderSize::new(width, height).is_forcible();
        let entry = Self::offered(&mut self.entries, id).filter(|_| valid)?;
        let CaptureStreamStage::Offered { age, asked } = entry.stage else {
            return None;
        };
        if asked && (entry.frame.width, entry.frame.height) != (width, height) {
            return None;
        }
        if !asked {
            let row_pitch = width.checked_mul(PixelFormat::BYTES_PER_PIXEL)?;
            let len = usize::try_from(row_pitch).ok()?.checked_mul(usize::try_from(height).ok()?)?;
            if entry.frame.pixels.capacity() == 0
                && let Some(spare) = self.spare.pop()
            {
                entry.frame.pixels = spare;
            }
            if entry.frame.pixels.len() != len && !Self::resize(&mut entry.frame.pixels, len) {
                curia::warn!("stream buffer not allocated", { id: id, bytes: len });
                entry.stage = CaptureStreamStage::Failed(CaptureStreamFault::CopyFailed);
                return None;
            }
            (entry.frame.width, entry.frame.height, entry.frame.row_pitch) = (width, height, row_pitch);
            entry.stage = CaptureStreamStage::Offered { age, asked: true };
        }
        let row_pitch = entry.frame.row_pitch;
        Some((entry.frame.pixels.as_mut_slice(), row_pitch))
    }

    /// The overlay's report for copy `id`: copied (`ok`, only once every row of its buffer is written) in the DXGI
    /// format `format`, or not. A good copy in a format `PixelFormat::from_dxgi` reads is ready to take; any other
    /// format fails `UnsupportedFormat`, and a failed copy or a good one with no buffer taken fails `CopyFailed`.
    /// False for an id not offered or already reported.
    pub fn report(&mut self, id: u32, ok: bool, format: u32) -> bool {
        let Some(entry) = Self::offered(&mut self.entries, id) else {
            return false;
        };
        let asked = matches!(entry.stage, CaptureStreamStage::Offered { asked: true, .. });
        entry.frame.format = format;
        entry.stage = match (ok, asked, PixelFormat::from_dxgi(format)) {
            (true, true, Some(_)) => CaptureStreamStage::Ready,
            (true, true, None) => {
                curia::warn!("stream frame refused, the back buffer format is not supported", { id: id, format: format });
                CaptureStreamStage::Failed(CaptureStreamFault::UnsupportedFormat { format })
            }
            _ => CaptureStreamStage::Failed(CaptureStreamFault::CopyFailed),
        };
        true
    }

    /// The device dropped its resources, the staging textures with them, for a device loss or a resize of the
    /// chain: every copy in flight is lost. The next Present fails it `Resized` when the back buffer size changed,
    /// else `DeviceLost`.
    pub fn device_lost(&mut self) {
        for entry in self.entries.iter_mut() {
            if matches!(entry.stage, CaptureStreamStage::Offered { .. }) {
                entry.stage = CaptureStreamStage::Lost;
            }
        }
    }

    /// The oldest frame of the mod in `index`, when it is done: its pixels, lent until the next take that returns
    /// a frame, or its failure. Either frees its slot. `None` while the oldest frame is still pending or in flight,
    /// or when the mod has no frame. It does not allocate: the slot keeps the buffer lent before.
    pub fn take(&mut self, index: ModIndex) -> Option<Result<&CaptureStreamFrame, CaptureStreamFailure>> {
        let entry = self
            .entries
            .iter_mut()
            .filter(|entry| entry.owner == Some(index) && entry.stage != CaptureStreamStage::Free)
            .min_by_key(|entry| entry.order)?;
        match entry.stage {
            CaptureStreamStage::Ready => {
                std::mem::swap(&mut entry.frame.pixels, &mut self.lent.pixels);
                let frame = &entry.frame;
                (self.lent.tag, self.lent.width, self.lent.height, self.lent.row_pitch, self.lent.format) =
                    (frame.tag, frame.width, frame.height, frame.row_pitch, frame.format);
                self.lent.present_seq = frame.present_seq;
                entry.free();
                Some(Ok(&self.lent))
            }
            CaptureStreamStage::Failed(fault) => {
                let failure = CaptureStreamFailure::new(entry.frame.tag, fault);
                entry.free();
                Some(Err(failure))
            }
            _ => None,
        }
    }

    /// The mod in `index` ends its stream, or unregisters: its frames are dropped, a copy in flight with them (the
    /// overlay's later report of it is refused). When no slot is in use after that, every pixel buffer, the lent one
    /// too, is freed, and the overlay is told at the next Present to drop its stream staging textures, which the
    /// next stream makes again. A close of a mod with no frame while another mod's stream runs changes nothing.
    ///
    /// Frames lent with `lend` stay with the mod: their buffers are kept until `give_back`, which then frees them.
    pub fn close(&mut self, index: ModIndex) {
        for entry in self.entries.iter_mut().filter(|entry| entry.owner == Some(index)) {
            entry.free();
        }
        for lent in self.lent_out.iter_mut().filter(|lent| lent.owner == Some(index)) {
            lent.orphaned = true;
        }
        if self.entries.iter().any(|entry| entry.stage != CaptureStreamStage::Free) {
            return;
        }
        for entry in self.entries.iter_mut() {
            entry.frame.pixels = Vec::new();
        }
        self.lent = CaptureStreamFrame::default();
        self.spare = Vec::new();
        self.release_due = true;
    }

    /// The mod in `index` unregisters: `close`, and every buffer lent to it is freed, so a later `give_back` of
    /// one is refused.
    pub fn unregister(&mut self, index: ModIndex) {
        self.lent_out.retain(|lent| lent.owner != Some(index));
        self.close(index);
    }

    /// The oldest frame of the mod in `index`, when it is done, as `take`, but the pixels are lent until
    /// `give_back` of the returned `lend_id`, however many frames are taken or lent in the meantime. A failed
    /// frame has no lease. `None` as for `take`, and also while `MAX_LENT` frames are lent and the oldest frame
    /// is a good one (it stays in the stream). It does not allocate once the slot buffers exist.
    pub fn lend(&mut self, index: ModIndex) -> Option<Result<&LentStreamFrame, CaptureStreamFailure>> {
        let at_limit = self.lent_out.len() >= Self::MAX_LENT;
        let entry = self
            .entries
            .iter_mut()
            .filter(|entry| entry.owner == Some(index) && entry.stage != CaptureStreamStage::Free)
            .min_by_key(|entry| entry.order)?;
        match entry.stage {
            CaptureStreamStage::Ready if !at_limit => {
                self.last_lend = self.last_lend.wrapping_add(1).max(1);
                let frame = &mut entry.frame;
                let lent = LentStreamFrame {
                    tag: frame.tag,
                    width: frame.width,
                    height: frame.height,
                    row_pitch: frame.row_pitch,
                    format: frame.format,
                    pixels: std::mem::take(&mut frame.pixels),
                    present_seq: frame.present_seq,
                    lend_id: self.last_lend,
                    owner: Some(index),
                    orphaned: false,
                };
                entry.free();
                self.lent_out.push(lent);
                self.lent_out.last().map(Ok)
            }
            CaptureStreamStage::Failed(fault) => {
                let failure = CaptureStreamFailure::new(entry.frame.tag, fault);
                entry.free();
                Some(Err(failure))
            }
            _ => None,
        }
    }

    /// The mod in `index` ends the lend `lend_id`: its buffer waits for a later frame, or is freed when the mod
    /// closed its stream since. False for an id the mod does not hold.
    pub fn give_back(&mut self, index: ModIndex, lend_id: u64) -> bool {
        let Some(position) = self.lent_out.iter().position(|lent| lent.lend_id == lend_id && lent.owner == Some(index))
        else {
            return false;
        };
        let lent = self.lent_out.swap_remove(position);
        if !lent.orphaned && self.spare.len() < Self::MAX_LENT {
            self.spare.push(lent.pixels);
        }
        true
    }

    /// The pixels of the lend `lend_id`, or `None` when it is not out.
    pub fn lent_pixels(&self, lend_id: u64) -> Option<&[u8]> {
        self.lent_out.iter().find(|lent| lent.lend_id == lend_id).map(|lent| lent.pixels.as_slice())
    }

    /// True once after `close` freed the buffers: this Present tells the overlay to drop its stream staging textures.
    pub fn take_release(&mut self) -> bool {
        std::mem::take(&mut self.release_due)
    }

    /// The bytes of pixel buffers the book holds, the lent ones and the spare ones included.
    pub fn held_bytes(&self) -> usize {
        self.entries.iter().map(|entry| entry.frame.pixels.capacity()).sum::<usize>()
            + self.lent.pixels.capacity()
            + self.lent_out.iter().map(|lent| lent.pixels.capacity()).sum::<usize>()
            + self.spare.iter().map(Vec::capacity).sum::<usize>()
    }

    /// True while a frame waits for a Present or for the overlay, or the overlay must be told to drop its stream
    /// textures, so the host must not answer a Present from its idle path.
    pub fn needs_present(&self) -> bool {
        self.release_due
            || self.entries.iter().any(|entry| {
                matches!(
                    entry.stage,
                    CaptureStreamStage::Pending | CaptureStreamStage::Offered { .. } | CaptureStreamStage::Lost
                )
            })
    }

    // True while a slot holds a frame of a mod other than `index`.
    fn held_by_other(&self, index: ModIndex) -> bool {
        self.entries.iter().any(|entry| entry.owner.is_some_and(|owner| owner != index))
    }

    fn offered(entries: &mut [CaptureStreamEntry], id: u32) -> Option<&mut CaptureStreamEntry> {
        entries.iter_mut().find(|entry| entry.id == id && matches!(entry.stage, CaptureStreamStage::Offered { .. }))
    }

    // Sets `pixels` to `len` bytes, false when the memory cannot be had. The bytes are zeroed once per size change,
    // never per frame of the same size.
    fn resize(pixels: &mut Vec<u8>, len: usize) -> bool {
        if pixels.capacity() < len && pixels.try_reserve_exact(len - pixels.len()).is_err() {
            return false;
        }
        pixels.resize(len, 0);
        true
    }
}

impl Default for CaptureStreamBook {
    fn default() -> Self {
        Self::new()
    }
}
