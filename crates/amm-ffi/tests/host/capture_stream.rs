use std::path::PathBuf;

use amm_abi::types::{
    AmmCapability, AmmCaptureStreamFault, AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmHandle, AmmStatus,
};
use amm_core::traits::AbiSized;
use amm_ffi::host::{amm_host_capture_done, amm_host_stream_buffer, amm_host_stream_done, amm_host_textures_lost};
use amm_ffi::services::PresentSequence;
use amm_ffi::types::{AMM_HOST_MAX_STREAM_FRAMES, AmmHostFrame};

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const RGBA: u32 = 28;
const FLOAT16: u32 = 10;
// a small back buffer keeps the painted pixels cheap; the stream must report this size
const BACK_BUFFER: (u32, u32) = (64, 36);
// the colour the game draws, and the colour of the marker the mod layers draw over it
const GAME: [u8; 4] = [10, 20, 30, 255];
const MARKER: [u8; 4] = [255, 0, 255, 255];

/// Stands in for the overlay's stream capture: it paints the back buffer the way `OverlayHook` draws a Present (the
/// game, the game-only copies, the mod layers, the layer copies), and fills the copies it made on the next
/// Present, before `amm_host_frame`, as `FrameCapture::poll` does after its non-blocking Map.
struct StreamOverlay {
    back_buffer: Vec<u8>,
    // (id, size, pixels) of each stream copy made on the last Present
    copies: Vec<(u32, (u32, u32), Vec<u8>)>,
    format: u32,
    size: (u32, u32),
    // the Presents whose host frame asked the overlay to drop its stream textures
    releases: u32,
}

impl StreamOverlay {
    fn new() -> Self {
        Self { back_buffer: Vec::new(), copies: Vec::new(), format: RGBA, size: BACK_BUFFER, releases: 0 }
    }

    fn in_format(format: u32) -> Self {
        Self { format, ..Self::new() }
    }

    // one Present: the copies of the last Present are read and reported, then the host frame is drawn
    fn present(&mut self, host: &TestHost, now_ms: u64) -> AmmHostFrame {
        self.report_copies();
        let frame = host.frame_sized(now_ms, self.size, self.size);
        if frame.release_stream {
            self.copies.clear();
            self.releases += 1;
        }
        let pixels = (self.size.0 * self.size.1) as usize;
        self.back_buffer = GAME.repeat(pixels);
        for id in streams(&frame) {
            self.copies.push((id, self.size, self.back_buffer.clone()));
        }
        if frame.layer_count > 0 {
            self.back_buffer = MARKER.repeat(pixels);
        }
        frame
    }

    // a resize of the chain: the overlay drops its staging textures with their copies, and tells the host
    fn resize(&mut self, size: (u32, u32)) {
        self.copies.clear();
        self.size = size;
        amm_host_textures_lost();
    }

    fn report_copies(&mut self) {
        for (id, (width, height), pixels) in self.copies.drain(..) {
            let mut pitch = 0;
            // SAFETY: the pitch lives for the call
            let buffer = unsafe { amm_host_stream_buffer(id, width, height, &mut pitch) };
            assert!(!buffer.is_null(), "no stream buffer for {id}");
            let row = (width * 4) as usize;
            for (y, source) in pixels.chunks(row).enumerate() {
                // SAFETY: the host gives height rows of pitch bytes at buffer until the done report, and a row of
                // width * 4 bytes fits in its pitch
                unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), buffer.add(y * pitch as usize), row) };
            }
            assert_eq!(amm_host_stream_done(id, 1, self.format), AmmStatus::Ok);
        }
    }
}

fn streams(frame: &AmmHostFrame) -> Vec<u32> {
    frame.streams[..frame.stream_count as usize].to_vec()
}

fn overlay_client(host: &TestHost, id: &str) -> TestClient {
    let client = TestClient::register(host, &Descriptors::overlay(id), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    client
}

fn request(handle: AmmHandle, tag: u64) -> AmmStatus {
    let call = TestHost::api().capture_stream_request.unwrap_or_else(|| panic!("capture_stream_request"));
    let request = AmmCaptureStreamRequest { tag, ..AmmCaptureStreamRequest::blank() };
    // SAFETY: the request lives for the call
    unsafe { call(handle, &request) }
}

fn free_slots(handle: AmmHandle) -> u32 {
    let call = TestHost::api().capture_stream_free.unwrap_or_else(|| panic!("capture_stream_free"));
    let mut free = u32::MAX;
    // SAFETY: the out count lives for the call
    assert_eq!(unsafe { call(handle, &mut free) }, AmmStatus::Ok);
    free
}

// the status of a take, and the frame with its pixels copied out while they are valid
fn take(handle: AmmHandle) -> (AmmStatus, AmmCaptureStreamFrame, Vec<u8>) {
    let call = TestHost::api().capture_stream_take.unwrap_or_else(|| panic!("capture_stream_take"));
    let mut frame = AmmCaptureStreamFrame::blank();
    // SAFETY: the out frame lives for the call
    let status = unsafe { call(handle, &mut frame) };
    let pixels = if frame.pixels.is_null() {
        Vec::new()
    } else {
        // SAFETY: the host lends pixels_len bytes at pixels until the next take that returns a frame
        unsafe { std::slice::from_raw_parts(frame.pixels, frame.pixels_len) }.to_vec()
    };
    (status, frame, pixels)
}

fn shot(host: &TestHost, name: &str) -> PathBuf {
    let dir = host.game_dir().join("shots");
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("shots dir: {error}"));
    dir.join(name)
}

#[test]
fn eight_tags_over_eight_presents_come_back_in_order_at_the_back_buffer_size() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    let mut taken = Vec::new();

    for tag in 1..=8u64 {
        assert_eq!(request(client.handle(), tag), AmmStatus::Ok);
        overlay.present(&host, tag);
        while let (AmmStatus::Ok, frame, _) = take(client.handle()) {
            taken.push(frame);
        }
    }
    for now in 9..12 {
        overlay.present(&host, now);
        while let (AmmStatus::Ok, frame, _) = take(client.handle()) {
            taken.push(frame);
        }
    }

    let tags: Vec<u64> = taken.iter().map(|frame| frame.tag).collect();
    assert_eq!(tags, (1..=8).collect::<Vec<u64>>());
    for frame in &taken {
        assert_eq!((frame.width, frame.height, frame.row_pitch), (BACK_BUFFER.0, BACK_BUFFER.1, BACK_BUFFER.0 * 4));
        assert_eq!(frame.format, RGBA);
        assert_eq!(frame.fault, AmmCaptureStreamFault::None as u8);
        assert_eq!(frame.pixels_len, (BACK_BUFFER.0 * 4 * BACK_BUFFER.1) as usize);
    }
}

#[test]
fn with_every_slot_full_the_next_request_is_busy_until_a_frame_is_taken() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    let ring = AMM_HOST_MAX_STREAM_FRAMES as u64;

    let free_before = free_slots(client.handle());
    for tag in 1..=ring {
        assert_eq!(request(client.handle(), tag), AmmStatus::Ok);
    }
    let full = request(client.handle(), ring + 1);
    let free_full = free_slots(client.handle());
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    let still_full = request(client.handle(), ring + 1);
    let (first, frame, _) = take(client.handle());
    let after_take = request(client.handle(), ring + 1);

    assert_eq!(free_before, ring as u32);
    assert_eq!(full, AmmStatus::Busy);
    assert_eq!(free_full, 0);
    assert_eq!(still_full, AmmStatus::Busy);
    assert_eq!((first, frame.tag), (AmmStatus::Ok, 1));
    assert_eq!(after_take, AmmStatus::Ok);
}

#[test]
fn a_stream_frame_never_holds_a_mod_layer() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    overlay.present(&host, 0);

    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    let frame = overlay.present(&host, 1);
    overlay.present(&host, 2);
    let (status, taken, pixels) = take(client.handle());

    assert!(frame.layer_count > 0, "the frame draws a mod layer");
    assert_eq!(status, AmmStatus::Ok);
    assert_eq!(taken.tag, 1);
    assert!(pixels.chunks(4).all(|pixel| pixel == GAME));
    assert!(!pixels.chunks(4).any(|pixel| pixel == MARKER));
}

#[test]
fn a_stream_frame_is_never_offered_in_a_screenshot_set_and_a_screenshot_keeps_its_own() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let shooter = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");
    let text = path.display().to_string();
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    // SAFETY: the path and the out sequence live for the call
    assert_eq!(unsafe { call(shooter.handle(), text.as_ptr(), text.len(), &mut sequence) }, AmmStatus::Ok);
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);

    let frame = host.frame_sized(0, BACK_BUFFER, BACK_BUFFER);

    assert_eq!(frame.captures[..frame.capture_count as usize], [sequence]);
    assert_eq!(frame.layer_capture_count, 0);
    assert_eq!(frame.stream_count, 1);
    // the two are apart: the stream copy's report does not end the screenshot, which still waits for its own
    assert_eq!(amm_host_stream_done(frame.streams[0], 0, RGBA), AmmStatus::Ok);
    assert_eq!(amm_host_capture_done(sequence, 0, RGBA), AmmStatus::Ok);
}

#[test]
fn a_frame_in_an_unsupported_format_fails_with_its_format() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::in_format(FLOAT16);

    assert_eq!(request(client.handle(), 7), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    let (status, frame, pixels) = take(client.handle());

    assert_eq!(status, AmmStatus::Ok);
    assert_eq!(frame.tag, 7);
    assert_eq!(frame.fault, AmmCaptureStreamFault::UnsupportedFormat as u8);
    assert_eq!(frame.format, FLOAT16);
    assert!(pixels.is_empty());
}

#[test]
fn a_device_loss_fails_the_frames_in_flight_at_the_next_present() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    assert_eq!(request(client.handle(), 3), AmmStatus::Ok);
    host.frame_sized(0, BACK_BUFFER, BACK_BUFFER);

    amm_host_textures_lost();
    let (before_present, _, _) = take(client.handle());
    host.frame_sized(1, BACK_BUFFER, BACK_BUFFER);
    let (status, frame, _) = take(client.handle());

    assert_eq!(before_present, AmmStatus::Busy);
    assert_eq!(status, AmmStatus::Ok);
    assert_eq!((frame.tag, frame.fault), (3, AmmCaptureStreamFault::DeviceLost as u8));
}

#[test]
fn a_frame_the_overlay_never_reports_times_out_and_later_frames_wait_behind_it() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    host.frame_sized(0, BACK_BUFFER, BACK_BUFFER);
    assert_eq!(request(client.handle(), 2), AmmStatus::Ok);
    let second = host.frame_sized(1, BACK_BUFFER, BACK_BUFFER).streams[0];
    let mut pitch = 0;
    // SAFETY: the pitch lives for the call
    let buffer = unsafe { amm_host_stream_buffer(second, BACK_BUFFER.0, BACK_BUFFER.1, &mut pitch) };
    assert!(!buffer.is_null());
    // SAFETY: the host gives height rows of pitch bytes at buffer until the done report
    unsafe { std::ptr::write_bytes(buffer, 0, (pitch * BACK_BUFFER.1) as usize) };
    assert_eq!(amm_host_stream_done(second, 1, RGBA), AmmStatus::Ok);
    host.frame_sized(2, BACK_BUFFER, BACK_BUFFER);

    let (waiting, _, _) = take(client.handle());
    for now in 3..40 {
        host.frame_sized(now, BACK_BUFFER, BACK_BUFFER);
    }
    let (first_status, first, _) = take(client.handle());
    let (second_status, second_frame, _) = take(client.handle());

    assert_eq!(waiting, AmmStatus::Busy);
    assert_eq!((first_status, first.tag, first.fault), (AmmStatus::Ok, 1, AmmCaptureStreamFault::TimedOut as u8));
    assert_eq!((second_status, second_frame.tag, second_frame.fault), (AmmStatus::Ok, 2, 0));
}

#[test]
fn a_request_needs_hud_or_overlay_and_another_mod_waits_while_the_stream_is_in_use() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let other = overlay_client(&host, "other");
    let unleased = TestClient::register(&host, &Descriptors::overlay("unleased"), TestMesh::triangle());

    let refused = request(unleased.handle(), 1);
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    let other_busy = request(other.handle(), 1);
    let other_free = free_slots(other.handle());

    assert_eq!(refused, AmmStatus::NotPermitted);
    assert_eq!(other_busy, AmmStatus::Busy);
    assert_eq!(other_free, 0);
}

#[test]
fn an_unregistered_mod_frees_the_stream_for_another_mod() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "render");
    let other = overlay_client(&host, "other");
    let mut overlay = StreamOverlay::new();
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);

    client.unregister();
    let free = free_slots(other.handle());
    let accepted = request(other.handle(), 9);
    overlay.present(&host, 2);
    overlay.present(&host, 3);
    let (status, frame, _) = take(other.handle());

    assert_eq!(free, AMM_HOST_MAX_STREAM_FRAMES as u32);
    assert_eq!(accepted, AmmStatus::Ok);
    assert_eq!((status, frame.tag), (AmmStatus::Ok, 9));
}

#[test]
fn the_stream_buffer_refuses_an_unknown_id_a_bad_size_and_a_null_pitch() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    let id = host.frame_sized(0, BACK_BUFFER, BACK_BUFFER).streams[0];
    let mut pitch = 0;

    // SAFETY: each pitch pointer is null on purpose or lives for the call
    let (unknown, zero, huge, null_pitch, good, other_size) = unsafe {
        (
            amm_host_stream_buffer(id + 100, 4, 4, &mut pitch),
            amm_host_stream_buffer(id, 0, 4, &mut pitch),
            amm_host_stream_buffer(id, 100_000, 4, &mut pitch),
            amm_host_stream_buffer(id, 4, 4, std::ptr::null_mut()),
            amm_host_stream_buffer(id, 4, 4, &mut pitch),
            amm_host_stream_buffer(id, 8, 4, &mut pitch),
        )
    };

    for refused in [unknown, zero, huge, null_pitch, other_size] {
        assert!(refused.is_null());
    }
    assert!(!good.is_null());
    assert_eq!(pitch, 16);
    assert_eq!(amm_host_stream_done(id + 100, 1, RGBA), AmmStatus::InvalidArgument);
    assert_eq!(amm_host_stream_done(id, 2, RGBA), AmmStatus::InvalidArgument);
}

#[test]
fn take_with_nothing_in_flight_is_busy_and_a_null_out_is_null_pointer() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let call = TestHost::api().capture_stream_take.unwrap_or_else(|| panic!("capture_stream_take"));
    let free = TestHost::api().capture_stream_free.unwrap_or_else(|| panic!("capture_stream_free"));

    let (nothing, _, _) = take(client.handle());
    // SAFETY: a null out pointer is never written
    let null_take = unsafe { call(client.handle(), std::ptr::null_mut()) };
    // SAFETY: a null out pointer is never written
    let null_free = unsafe { free(client.handle(), std::ptr::null_mut()) };

    assert_eq!(nothing, AmmStatus::Busy);
    assert_eq!(null_take, AmmStatus::NullPointer);
    assert_eq!(null_free, AmmStatus::NullPointer);
}

#[test]
fn the_capture_stream_slots_are_filled() {
    let api = TestHost::api();

    assert!(api.capture_stream_close.is_some());
    assert!(api.capture_stream_request.is_some());
    assert!(api.capture_stream_take.is_some());
    assert!(api.capture_stream_free.is_some());
    assert!(api.present_count.is_some());
}

fn close(handle: AmmHandle) -> AmmStatus {
    let call = TestHost::api().capture_stream_close.unwrap_or_else(|| panic!("capture_stream_close"));
    // SAFETY: close takes a plain handle
    unsafe { call(handle) }
}

// the bytes of stream pixel buffers the host holds
fn stream_bytes(host: &TestHost) -> usize {
    let mut bytes = usize::MAX;
    host.with_runtime(|runtime| bytes = runtime.stream_bytes());
    bytes
}

#[test]
fn close_frees_the_stream_buffers_and_textures_once_and_a_new_stream_starts() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    let frame_bytes = (BACK_BUFFER.0 * BACK_BUFFER.1 * 4) as usize;
    for tag in 1..=3 {
        assert_eq!(request(client.handle(), tag), AmmStatus::Ok);
    }
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    let (first, _, _) = take(client.handle());
    let held = stream_bytes(&host);

    let closed = close(client.handle());
    let held_after = stream_bytes(&host);
    overlay.present(&host, 2);
    overlay.present(&host, 3);
    let restarted = request(client.handle(), 10);
    overlay.present(&host, 4);
    overlay.present(&host, 5);
    let (status, frame, pixels) = take(client.handle());

    assert_eq!(first, AmmStatus::Ok);
    assert!(held >= frame_bytes * 3);
    assert_eq!(closed, AmmStatus::Ok);
    assert_eq!(held_after, 0);
    assert_eq!(overlay.releases, 1);
    assert_eq!(restarted, AmmStatus::Ok);
    assert_eq!((status, frame.tag, pixels.len()), (AmmStatus::Ok, 10, frame_bytes));
}

#[test]
fn a_screenshot_after_a_closed_stream_keeps_no_stream_memory() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    assert_eq!(close(client.handle()), AmmStatus::Ok);
    let path = shot(&host, "after.png");
    let text = path.display().to_string();
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    // SAFETY: the path and the out sequence live for the call
    assert_eq!(unsafe { call(client.handle(), text.as_ptr(), text.len(), &mut sequence) }, AmmStatus::Ok);

    let frame = overlay.present(&host, 2);

    assert_eq!(frame.capture_count, 1);
    assert_eq!(frame.stream_count, 0);
    assert_eq!(stream_bytes(&host), 0);
}

#[test]
fn unregister_frees_the_stream_buffers_and_textures() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "render");
    let _other = overlay_client(&host, "other");
    let mut overlay = StreamOverlay::new();
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);

    client.unregister();
    let held = stream_bytes(&host);
    overlay.present(&host, 2);

    assert_eq!(held, 0);
    assert_eq!(overlay.releases, 1);
}

#[test]
fn a_resize_mid_stream_fails_the_frames_in_flight_as_resized_in_order_and_later_frames_take_the_new_size() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    let bigger = (96, 54);
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    assert_eq!(request(client.handle(), 2), AmmStatus::Ok);
    overlay.present(&host, 0);

    overlay.resize(bigger);
    assert_eq!(request(client.handle(), 3), AmmStatus::Ok);
    overlay.present(&host, 1);
    overlay.present(&host, 2);
    let taken: Vec<(AmmStatus, AmmCaptureStreamFrame, Vec<u8>)> = (0..3).map(|_| take(client.handle())).collect();

    let faults: Vec<(u64, u8)> = taken.iter().map(|(_, frame, _)| (frame.tag, frame.fault)).collect();
    let resized = AmmCaptureStreamFault::Resized as u8;
    assert_eq!(faults, vec![(1, resized), (2, resized), (3, 0)]);
    let (status, third, pixels) = &taken[2];
    assert_eq!(*status, AmmStatus::Ok);
    assert_eq!((third.width, third.height, third.row_pitch), (bigger.0, bigger.1, bigger.0 * 4));
    assert_eq!(pixels.len(), (bigger.0 * bigger.1 * 4) as usize);
}

#[test]
fn a_take_with_a_size_below_base_is_invalid_and_keeps_the_frame() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    overlay.present(&host, 0);
    assert_eq!(request(client.handle(), 7), AmmStatus::Ok);
    overlay.present(&host, 1);
    overlay.present(&host, 2);
    let call = TestHost::api().capture_stream_take.unwrap_or_else(|| panic!("capture_stream_take"));
    let mut zeroed = AmmCaptureStreamFrame { size: 0, ..AmmCaptureStreamFrame::blank() };

    // SAFETY: the out frame lives for the call
    let refused = unsafe { call(client.handle(), &mut zeroed) };
    let (status, frame, _) = take(client.handle());

    assert_eq!(refused, AmmStatus::InvalidArgument);
    assert_eq!(zeroed.size, 0);
    assert_eq!((status, frame.tag), (AmmStatus::Ok, 7));
}

fn present_count(handle: AmmHandle, out: *mut u64) -> AmmStatus {
    let call = TestHost::api().present_count.unwrap_or_else(|| panic!("present_count"));
    // SAFETY: the out count lives for the call, or is null, which the call refuses
    unsafe { call(handle, out) }
}

#[test]
fn present_count_reads_the_overlay_count() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    PresentSequence::next();
    PresentSequence::next();
    let mut count = 0u64;

    let status = present_count(client.handle(), &mut count);

    assert_eq!((status, count), (AmmStatus::Ok, 2));
}

#[test]
fn present_count_null_out_is_null_pointer() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");

    assert_eq!(present_count(client.handle(), std::ptr::null_mut()), AmmStatus::NullPointer);
}

// the status of a lend, and the frame with its pixels read in place while they are lent
fn lend(handle: AmmHandle) -> (AmmStatus, AmmCaptureStreamFrame) {
    let call = TestHost::api().capture_stream_lend.unwrap_or_else(|| panic!("capture_stream_lend"));
    let mut frame = AmmCaptureStreamFrame::blank();
    // SAFETY: the out frame lives for the call
    let status = unsafe { call(handle, &mut frame) };
    (status, frame)
}

fn give_back(handle: AmmHandle, lend_id: u64) -> AmmStatus {
    let call = TestHost::api().capture_stream_give_back.unwrap_or_else(|| panic!("capture_stream_give_back"));
    // SAFETY: give_back takes a handle and a plain id
    unsafe { call(handle, lend_id) }
}

fn lent_pixels(frame: &AmmCaptureStreamFrame) -> &[u8] {
    // SAFETY: the host lends pixels_len bytes at pixels until the frame is given back
    unsafe { std::slice::from_raw_parts(frame.pixels, frame.pixels_len) }
}

#[test]
fn a_lent_frame_keeps_its_pixels_across_a_later_take_and_lend() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    for tag in 1..=3u64 {
        assert_eq!(request(client.handle(), tag), AmmStatus::Ok);
        overlay.present(&host, tag);
    }
    overlay.present(&host, 4);
    overlay.present(&host, 5);

    let (first_status, first) = lend(client.handle());
    let (taken_status, taken, _) = take(client.handle());
    let (second_status, second) = lend(client.handle());

    assert_eq!((first_status, taken_status, second_status), (AmmStatus::Ok, AmmStatus::Ok, AmmStatus::Ok));
    assert_eq!((first.tag, taken.tag, second.tag), (1, 2, 3));
    assert_ne!(first.lend_id, 0);
    assert_ne!(first.lend_id, second.lend_id);
    assert_eq!(taken.lend_id, 0);
    assert_eq!(first.pixels_len, (BACK_BUFFER.0 * 4 * BACK_BUFFER.1) as usize);
    assert!(lent_pixels(&first).chunks(4).all(|pixel| pixel == GAME));
    assert!(lent_pixels(&second).chunks(4).all(|pixel| pixel == GAME));
    assert_eq!(give_back(client.handle(), first.lend_id), AmmStatus::Ok);
    assert_eq!(give_back(client.handle(), second.lend_id), AmmStatus::Ok);
}

#[test]
fn give_back_of_an_unknown_id_is_invalid_argument() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    let (status, frame) = lend(client.handle());

    let unknown = give_back(client.handle(), frame.lend_id + 50);
    let known = give_back(client.handle(), frame.lend_id);
    let twice = give_back(client.handle(), frame.lend_id);

    assert_eq!(status, AmmStatus::Ok);
    assert_eq!(unknown, AmmStatus::InvalidArgument);
    assert_eq!(known, AmmStatus::Ok);
    assert_eq!(twice, AmmStatus::InvalidArgument);
}

#[test]
fn lend_with_nothing_ready_is_busy_and_a_zero_size_is_refused() {
    let host = TestHost::new();
    let client = overlay_client(&host, "render");
    let (nothing, _) = lend(client.handle());
    let call = TestHost::api().capture_stream_lend.unwrap_or_else(|| panic!("capture_stream_lend"));
    let mut zeroed = AmmCaptureStreamFrame { size: 0, ..AmmCaptureStreamFrame::blank() };

    // SAFETY: the out frame lives for the call
    let refused = unsafe { call(client.handle(), &mut zeroed) };
    // SAFETY: a null out frame is refused
    let null = unsafe { call(client.handle(), std::ptr::null_mut()) };

    assert_eq!(nothing, AmmStatus::Busy);
    assert_eq!(refused, AmmStatus::InvalidArgument);
    assert_eq!(null, AmmStatus::NullPointer);
}

#[test]
fn an_unregistered_mod_loses_its_lent_buffers() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "render");
    let mut overlay = StreamOverlay::new();
    assert_eq!(request(client.handle(), 1), AmmStatus::Ok);
    overlay.present(&host, 0);
    overlay.present(&host, 1);
    let handle = client.handle();
    let (status, frame) = lend(handle);
    let held = stream_bytes(&host);

    client.unregister();

    assert_eq!(status, AmmStatus::Ok);
    assert_ne!(frame.lend_id, 0);
    assert!(held > 0);
    assert_eq!(stream_bytes(&host), 0);
    assert_eq!(give_back(handle, frame.lend_id), AmmStatus::UnknownHandle);
}
