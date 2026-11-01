/// One game-only frame of the capture stream, taken before any mod layer. `pixels` holds `height` rows of
/// `row_pitch` bytes; the first `width * 4` bytes of a row are its pixels in the DXGI format `format`, which is one
/// that `PixelFormat::from_dxgi` reads. The pixels are not converted. `present_seq` is the number of the Present that
/// filled the frame (the overlay's count of the game's Presents, from 1; 0 when the manager does not count).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CaptureStreamFrame {
    pub tag: u64,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub format: u32,
    pub pixels: Vec<u8>,
    pub present_seq: u64,
}
