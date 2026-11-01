use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN};

/// One staging texture of the frame capture pool and the capture it holds. The texture is kept between captures
/// of the same size and format and made again when either changes.
pub(crate) struct StagingSlot {
    pub texture: Option<ID3D11Texture2D>,
    pub width: u32,
    pub height: u32,
    pub format: DXGI_FORMAT,
    pub sequence: u32,
    // a copy is issued into the texture and not reported yet
    pub busy: bool,
    // the Presents the copy has waited for its non-blocking Map
    pub waited: u32,
}

impl StagingSlot {
    pub fn new() -> Self {
        Self { texture: None, width: 0, height: 0, format: DXGI_FORMAT_UNKNOWN, sequence: 0, busy: false, waited: 0 }
    }

    /// Ends the capture the slot holds and keeps its texture.
    pub fn free(&mut self) {
        self.busy = false;
        self.sequence = 0;
        self.waited = 0;
    }

    /// Ends the capture and drops the texture.
    pub fn reset(&mut self) {
        self.free();
        self.texture = None;
        self.width = 0;
        self.height = 0;
        self.format = DXGI_FORMAT_UNKNOWN;
    }
}
