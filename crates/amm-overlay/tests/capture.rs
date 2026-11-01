use amm_overlay::{CaptureKind, CaptureSink, FrameCapture};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM};

use super::support::WarpTarget;

/// Hands out one buffer of `pitch`-byte rows filled with a marker byte, or none, and records each report.
struct TestSink {
    give_buffer: bool,
    pitch: u32,
    buffer: Vec<u8>,
    asked: Vec<(u32, u32, u32)>,
    done: Vec<(u32, bool, DXGI_FORMAT)>,
}

impl TestSink {
    const MARKER: u8 = 0xAB;

    fn new(give_buffer: bool, pitch: u32) -> Self {
        Self { give_buffer, pitch, buffer: Vec::new(), asked: Vec::new(), done: Vec::new() }
    }

    /// Polls until a capture is reported, as the overlay does once per Present.
    fn poll_until_done(&mut self, capture: &mut FrameCapture<4>, target: &WarpTarget) {
        for _ in 0..FrameCapture::<4>::MAP_WAIT_FRAMES {
            capture.poll(&target.ctx, self);
            if !self.done.is_empty() {
                return;
            }
            // SAFETY: the context is live
            unsafe { target.ctx.Flush() };
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

impl CaptureSink for TestSink {
    fn buffer(&mut self, sequence: u32, width: u32, height: u32) -> Option<(&mut [u8], u32)> {
        self.asked.push((sequence, width, height));
        if !self.give_buffer {
            return None;
        }
        self.buffer = vec![Self::MARKER; (self.pitch * height) as usize];
        Some((&mut self.buffer, self.pitch))
    }

    fn done(&mut self, sequence: u32, ok: bool, format: DXGI_FORMAT) {
        self.done.push((sequence, ok, format));
    }
}

#[test]
fn capture_of_the_target_fills_every_row_and_reports_its_format() {
    let target = WarpTarget::new();
    target.clear([10, 20, 30, 255]);
    let mut capture = FrameCapture::<4>::new(CaptureKind::Screenshot);
    let row_bytes = (WarpTarget::SIZE * 4) as usize;
    let mut sink = TestSink::new(true, row_bytes as u32 + 16);

    capture.request(&target.device, &target.ctx, &target.resource(), &[5], &mut sink);
    assert!(capture.in_flight());
    sink.poll_until_done(&mut capture, &target);

    assert_eq!(sink.asked, vec![(5, WarpTarget::SIZE, WarpTarget::SIZE)]);
    assert_eq!(sink.done, vec![(5, true, DXGI_FORMAT_R8G8B8A8_UNORM)]);
    assert!(!capture.in_flight());
    for row in sink.buffer.chunks_exact(sink.pitch as usize) {
        let (pixels, padding) = row.split_at(row_bytes);
        assert_eq!(pixels.len(), 64 * 4);
        assert!(pixels.as_chunks::<4>().0.iter().all(|pixel| *pixel == [10, 20, 30, 255]));
        assert!(padding.iter().all(|byte| *byte == TestSink::MARKER));
    }
}

#[test]
fn capture_without_a_buffer_is_reported_failed() {
    let target = WarpTarget::new();
    let mut capture = FrameCapture::<4>::new(CaptureKind::Screenshot);
    let mut sink = TestSink::new(false, 0);

    capture.request(&target.device, &target.ctx, &target.resource(), &[9], &mut sink);
    sink.poll_until_done(&mut capture, &target);

    assert_eq!(sink.done, vec![(9, false, DXGI_FORMAT_R8G8B8A8_UNORM)]);
    assert!(!capture.in_flight());
}

#[test]
fn sequences_past_the_pool_are_not_copied() {
    let target = WarpTarget::new();
    let mut capture = FrameCapture::<4>::new(CaptureKind::Screenshot);
    let mut sink = TestSink::new(true, WarpTarget::SIZE * 4);

    capture.request(&target.device, &target.ctx, &target.resource(), &[1, 2, 3, 4, 5], &mut sink);
    capture.request(&target.device, &target.ctx, &target.resource(), &[6], &mut sink);

    // the fifth sequence is cut off; the sixth finds every staging texture busy
    assert_eq!(sink.done, vec![(6, false, DXGI_FORMAT_R8G8B8A8_UNORM)]);
    assert!(capture.release());
    assert!(!capture.in_flight());
}

#[test]
fn the_stream_pool_holds_its_own_slots_apart_from_the_screenshot_pool() {
    // a full stream pool still leaves every screenshot slot free
    let target = WarpTarget::new();
    let mut screenshots = FrameCapture::<4>::new(CaptureKind::Screenshot);
    let mut stream = FrameCapture::<4>::new(CaptureKind::Stream);
    let mut sink = TestSink::new(true, WarpTarget::SIZE * 4);

    stream.request(&target.device, &target.ctx, &target.resource(), &[1, 2, 3, 4], &mut sink);
    screenshots.request(&target.device, &target.ctx, &target.resource(), &[9], &mut sink);

    assert!(stream.in_flight());
    assert!(screenshots.in_flight());
    assert!(sink.done.is_empty());
}

#[test]
fn a_fifth_stream_id_in_one_request_is_cut_with_no_report() {
    let target = WarpTarget::new();
    let mut stream = FrameCapture::<4>::new(CaptureKind::Stream);
    let mut sink = TestSink::new(true, WarpTarget::SIZE * 4);

    stream.request(&target.device, &target.ctx, &target.resource(), &[1, 2, 3, 4, 5], &mut sink);
    for _ in 0..FrameCapture::<4>::MAP_WAIT_FRAMES {
        stream.poll(&target.ctx, &mut sink);
        if !stream.in_flight() {
            break;
        }
        // SAFETY: the context is live
        unsafe { target.ctx.Flush() };
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let mut reported: Vec<u32> = sink.done.iter().map(|(id, _, _)| *id).collect();
    reported.sort();
    assert_eq!(reported, vec![1, 2, 3, 4]);
    assert!(sink.asked.iter().all(|(id, _, _)| *id != 5));
}
