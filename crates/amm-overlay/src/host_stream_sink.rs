use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

use crate::capture_sink::CaptureSink;
use crate::overlay_host::OverlayHost;

/// Hands the capture stream frames of `FrameCapture` to the manager through its `OverlayHost`. Lives for one
/// Present.
pub(crate) struct HostStreamSink<'a> {
    host: &'a dyn OverlayHost,
}

impl<'a> HostStreamSink<'a> {
    pub(crate) fn new(host: &'a dyn OverlayHost) -> Self {
        Self { host }
    }
}

impl CaptureSink for HostStreamSink<'_> {
    fn buffer(&mut self, sequence: u32, width: u32, height: u32) -> Option<(&mut [u8], u32)> {
        let (buffer, row_pitch) = self.host.stream_buffer(sequence, width, height)?;
        let len = (height as usize).checked_mul(row_pitch as usize)?;
        // SAFETY: OverlayHost promises the buffer is writable for `height * row_pitch` bytes and unused by anything
        // else until the stream frame is done; the slice is dropped inside this Present, before `done`
        Some((unsafe { std::slice::from_raw_parts_mut(buffer.as_ptr(), len) }, row_pitch))
    }

    fn done(&mut self, sequence: u32, ok: bool, format: DXGI_FORMAT) {
        self.host.stream_done(sequence, ok, format);
    }
}
