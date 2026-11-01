/// One capture stream frame whose pixel buffer the manager lent to the mod: `pixels` reads it in place, with no
/// copy. The buffer is the mod's until `CaptureStream::give_back` with `lend_id`; the manager never writes a lent
/// buffer, so the frame may move to another thread. Nothing ties the frame to the lease, so `pixels` is `unsafe`:
/// after `give_back`, after the link is dropped, or after the manager unregisters the mod, the pixels are gone and
/// must not be read. At most 8 frames are lent at once.
#[derive(Debug)]
pub struct LentStreamFrame {
    pub tag: u64,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub format: u32,
    pub present_seq: u64,
    pub lend_id: u64,
    pixels: *const u8,
    pixels_len: usize,
}

// SAFETY: moving the frame to another thread only moves the pointer; the buffer is not written or moved by the
// manager while it is lent, so a read from the other thread is a read of shared bytes. The reader upholds the
// contract of `pixels` (lease out, link alive and registered) on whatever thread it reads.
unsafe impl Send for LentStreamFrame {}

impl LentStreamFrame {
    pub(crate) fn new(
        (tag, width, height, row_pitch, format): (u64, u32, u32, u32, u32),
        (present_seq, lend_id): (u64, u64),
        pixels: *const u8,
        pixels_len: usize,
    ) -> Self {
        Self { tag, width, height, row_pitch, format, present_seq, lend_id, pixels, pixels_len }
    }

    /// The frame's pixels, as the manager wrote them: `height` rows of `row_pitch` bytes, of which the first
    /// `width * 4` are the row's pixels in the DXGI format `format`. Empty for a frame with no buffer.
    ///
    /// # Safety
    ///
    /// Call it, and use the slice, only while the lease is out: before `CaptureStream::give_back` of this frame's
    /// `lend_id` runs, and while the `ManagerLink` and `CaptureStream` it came from are alive and the mod is
    /// registered. After any of these ends the buffer is reused or freed, and a read is a data race or a
    /// use-after-free.
    pub unsafe fn pixels(&self) -> &[u8] {
        if self.pixels.is_null() {
            return &[];
        }
        // SAFETY: the manager lends pixels_len readable bytes at the non-null pixels while the lease is out, which
        // the caller guarantees
        unsafe { std::slice::from_raw_parts(self.pixels, self.pixels_len) }
    }
}
