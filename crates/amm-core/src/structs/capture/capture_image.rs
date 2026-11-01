use std::path::PathBuf;

/// The pixels of one capture, handed to the I/O thread to be written as a PNG at `path`. `pixels` holds
/// `height` rows of `row_pitch` bytes each; the first `width * 4` bytes of a row are its pixels, in the DXGI
/// format `format`. The buffer is made once per capture, never per frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureImage {
    pub sequence: u32,
    pub path: PathBuf,
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub format: u32,
}
