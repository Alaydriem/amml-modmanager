use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

/// Where `FrameCapture` hands the pixels of a capture: the manager's capture relay in the game, a test buffer in
/// tests. Render thread, inside Present.
pub trait CaptureSink {
    /// The buffer for the pixels of capture `sequence` and its row pitch in bytes: `height` rows, of which the
    /// first `width * 4` bytes of each are written. `None` when the sink has no buffer for it; the capture then
    /// fails.
    fn buffer(&mut self, sequence: u32, width: u32, height: u32) -> Option<(&mut [u8], u32)>;

    /// Capture `sequence` was written in full to its buffer in `format` (`ok` true), or could not be.
    fn done(&mut self, sequence: u32, ok: bool, format: DXGI_FORMAT);
}
