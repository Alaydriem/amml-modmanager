use amm_core::traits::AbiSized;

/// One frame of the capture stream, written by `capture_stream_take`. The mod sets `size` to `size_of` its struct;
/// the manager writes only that many bytes and sets `size` to the bytes it wrote. With `fault` `AmmCaptureStreamFault::None`,
/// `pixels` holds `pixels_len` bytes: `height` rows of `row_pitch` bytes, of which the first `width * 4` are the
/// row's pixels in the DXGI format `format`, as the game drew them before any mod layer. The manager lends the
/// pixels until the next `capture_stream_take` that returns a frame, `capture_stream_close`, or until the mod
/// unregisters. With any other
/// `fault` the frame failed: `pixels` is null, `pixels_len` 0, and `format` is the back buffer's format for
/// `UnsupportedFormat`, else 0. `present_seq` (after `pixels_len`) is the number of the Present that filled the frame, counted by the overlay from 1; 0 when the manager
/// does not count. `lend_id` (after `present_seq`) is 0 for a frame from `capture_stream_take`; a frame from
/// `capture_stream_lend` names its lease there, and its pixels stay valid until `capture_stream_give_back` with it.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmCaptureStreamFrame {
    pub size: u32,
    pub tag: u64,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub format: u32,
    pub fault: u8,
    pub pixels: *const u8,
    pub pixels_len: usize,
    pub present_seq: u64,
    pub lend_id: u64,
}

impl AbiSized for AmmCaptureStreamFrame {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            tag: 0,
            width: 0,
            height: 0,
            row_pitch: 0,
            format: 0,
            fault: 0,
            pixels: std::ptr::null(),
            pixels_len: 0,
            present_seq: 0,
            lend_id: 0,
        }
    }
}
