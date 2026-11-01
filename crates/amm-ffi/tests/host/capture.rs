use std::path::PathBuf;
use std::time::Duration;

use amm_abi::types::{AmmCapability, AmmEvent, AmmEventKind, AmmHandle, AmmStatus};
use amm_core::services::capture::CaptureBook;
use amm_ffi::host::{amm_host_capture_buffer, amm_host_capture_done, amm_host_textures_lost, amm_host_tick};
use amm_ffi::services::HostCell;
use amm_ffi::types::{AMM_HOST_CAPTURE_MAX_FRAMES, AmmHostFrame};

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const RGBA: u32 = 28;
const BGRA: u32 = 87;
const FLOAT16: u32 = 10;
const FRAME_LIMIT: u64 = AMM_HOST_CAPTURE_MAX_FRAMES as u64;

fn overlay_client(host: &TestHost, id: &str) -> TestClient {
    let client = TestClient::register(host, &Descriptors::overlay(id), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    client
}

fn shot(host: &TestHost, name: &str) -> PathBuf {
    let dir = host.game_dir().join("shots");
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("shots dir: {error}"));
    dir.join(name)
}

fn request(handle: AmmHandle, path: &std::path::Path) -> u32 {
    let text = path.display().to_string();
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    // SAFETY: the path and the out sequence live for the call
    let status = unsafe { call(handle, text.as_ptr(), text.len(), &mut sequence) };
    assert_eq!(status, AmmStatus::Ok);
    sequence
}

fn request_layers(handle: AmmHandle, path: &std::path::Path, layers: u8) -> u32 {
    let text = path.display().to_string();
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot_layers.unwrap_or_else(|| panic!("request_screenshot_layers"));
    // SAFETY: the path and the out sequence live for the call
    let status = unsafe { call(handle, text.as_ptr(), text.len(), layers, &mut sequence) };
    assert_eq!(status, AmmStatus::Ok);
    sequence
}

fn capture_buffer(sequence: u32, width: u32, height: u32, out_row_pitch: *mut u32) -> *mut u8 {
    // SAFETY: out_row_pitch is null on purpose or a live u32 of the caller for the call
    unsafe { amm_host_capture_buffer(sequence, width, height, out_row_pitch) }
}

// True while the host keeps every Present off its lock-free idle path for a capture.
fn capturing() -> bool {
    HostCell::global().shared().is_some_and(|shared| shared.capturing())
}

fn captures(frame: &AmmHostFrame) -> Vec<u32> {
    frame.captures[..frame.capture_count as usize].to_vec()
}

fn layer_captures(frame: &AmmHostFrame) -> Vec<u32> {
    frame.layer_captures[..frame.layer_capture_count as usize].to_vec()
}

// Copies `pixels` (rows of `width * 4` bytes) into the buffer the host gives for the capture.
fn fill(sequence: u32, width: u32, height: u32, pixels: &[u8]) -> bool {
    let mut pitch = 0;
    let buffer = capture_buffer(sequence, width, height, &mut pitch);
    if buffer.is_null() {
        return false;
    }
    let row = (width * 4) as usize;
    for (y, source) in pixels.chunks(row).enumerate() {
        // SAFETY: the host gives height rows of pitch bytes at buffer, valid until capture_done, and each row of
        // width * 4 bytes fits in its pitch
        unsafe { std::ptr::copy_nonoverlapping(source.as_ptr(), buffer.add(y * pitch as usize), row) };
    }
    true
}

fn screenshot_events(client: &TestClient) -> Vec<(AmmEventKind, u32, u8)> {
    let kept = |event: &AmmEvent| {
        matches!(event.kind, AmmEventKind::ScreenshotTaken | AmmEventKind::ScreenshotWritten).then_some((
            event.kind,
            event.code,
            event.phase,
        ))
    };
    client.poll().iter().filter_map(kept).collect()
}

// Ticks until the I/O thread reports the write, or gives up after a few seconds.
fn wait_for_written(client: &TestClient, now_ms: u64) -> Vec<(AmmEventKind, u32, u8)> {
    let mut seen = Vec::new();
    for step in 0..500 {
        amm_host_tick(now_ms + step);
        seen.extend(screenshot_events(client));
        if seen.iter().any(|(kind, _, _)| *kind == AmmEventKind::ScreenshotWritten) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    seen
}

#[test]
fn a_frame_with_no_capture_pending_offers_none() {
    let host = TestHost::new();
    let _client = overlay_client(&host, "shooter");

    let frame = host.frame(0);

    assert_eq!(frame.capture_count, 0);
}

#[test]
fn a_requested_capture_is_offered_once_then_written_as_a_png_with_both_events() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");
    let sequence = request(client.handle(), &path);

    let offered = captures(&host.frame(0));
    let again = captures(&host.frame(1));
    assert!(fill(sequence, 2, 1, &[30, 20, 10, 255, 70, 60, 50, 255]));
    assert_eq!(amm_host_capture_done(sequence, 1, BGRA), AmmStatus::Ok);
    host.frame(2);
    let events = wait_for_written(&client, 3);

    assert_eq!(offered, vec![sequence]);
    assert!(again.is_empty());
    assert_eq!(
        events,
        vec![(AmmEventKind::ScreenshotTaken, sequence, 1), (AmmEventKind::ScreenshotWritten, sequence, 1)]
    );
    assert_eq!(&std::fs::read(&path).unwrap_or_default()[..4], b"\x89PNG");
    assert!(!capturing());
}

#[test]
fn a_copy_the_overlay_reports_failed_ends_with_taken_not_ok_and_no_file() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");
    let sequence = request(client.handle(), &path);
    host.frame(0);

    assert_eq!(amm_host_capture_done(sequence, 0, RGBA), AmmStatus::Ok);
    host.frame(1);

    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
    assert!(!path.exists());
    assert!(!capturing());
}

#[test]
fn a_buffer_asked_for_and_not_reported_by_the_next_frame_fails_the_capture() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    host.frame(0);
    assert!(fill(sequence, 1, 1, &[1, 2, 3, 4]));
    let capturing_before = capturing();

    host.frame(1);
    let late_done = amm_host_capture_done(sequence, 1, RGBA);

    assert!(capturing_before);
    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
    assert_eq!(late_done, AmmStatus::InvalidArgument);
    assert!(!capturing());
}

#[test]
fn an_unsupported_back_buffer_format_ends_with_taken_not_ok_and_no_file() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");
    let sequence = request(client.handle(), &path);
    host.frame(0);
    assert!(fill(sequence, 1, 1, &[0; 4]));

    assert_eq!(amm_host_capture_done(sequence, 1, FLOAT16), AmmStatus::Ok);
    host.frame(1);

    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
    assert!(!path.exists());
}

#[test]
fn a_capture_not_reported_within_thirty_frames_fails_and_frees_its_slot() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    host.frame(0);

    for now in 1..FRAME_LIMIT {
        host.frame(now);
    }
    let before_limit = screenshot_events(&client);
    host.frame(FRAME_LIMIT);
    host.frame(FRAME_LIMIT + 1);
    let late_done = amm_host_capture_done(sequence, 1, RGBA);

    assert!(before_limit.is_empty());
    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
    assert_eq!(late_done, AmmStatus::InvalidArgument);
    assert_ne!(request(client.handle(), &shot(&host, "b.png")), sequence);
}

#[test]
fn a_device_loss_fails_every_capture_the_overlay_has_not_reported() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    host.frame(0);

    amm_host_textures_lost();
    host.frame(1);

    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
}

#[test]
fn the_capture_buffer_refuses_an_unknown_sequence_a_bad_size_and_a_null_pitch() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    let mut pitch = 0;
    let not_taken = capture_buffer(sequence, 4, 4, &mut pitch);
    host.frame(0);

    let unknown = capture_buffer(sequence + 100, 4, 4, &mut pitch);
    let zero = capture_buffer(sequence, 0, 4, &mut pitch);
    let huge = capture_buffer(sequence, 100_000, 4, &mut pitch);
    let null_pitch = capture_buffer(sequence, 4, 4, std::ptr::null_mut());
    let good = capture_buffer(sequence, 4, 4, &mut pitch);
    let other_size = capture_buffer(sequence, 8, 4, &mut pitch);

    for refused in [not_taken, unknown, zero, huge, null_pitch, other_size] {
        assert!(refused.is_null());
    }
    assert!(!good.is_null());
    assert_eq!(pitch, 16);
}

#[test]
fn a_done_report_without_pixels_or_for_an_unknown_sequence_is_refused_or_failed() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    host.frame(0);

    let unknown = amm_host_capture_done(sequence + 100, 1, RGBA);
    let no_pixels = amm_host_capture_done(sequence, 1, RGBA);
    host.frame(1);

    assert_eq!(unknown, AmmStatus::InvalidArgument);
    assert_eq!(no_pixels, AmmStatus::Ok);
    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
}

#[test]
fn a_capture_of_a_mod_that_unregisters_after_the_copy_starts_writes_no_file_and_reaches_no_slot_reuser() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");
    let sequence = request(client.handle(), &path);
    host.frame(0);
    assert!(fill(sequence, 1, 1, &[1, 2, 3, 4]));

    let first_slot = client.owner();
    client.unregister();
    let next = overlay_client(&host, "next");
    assert_eq!(amm_host_capture_done(sequence, 1, RGBA), AmmStatus::Ok);
    host.frame(1);
    let mut next_events = Vec::new();
    for step in 0..50 {
        amm_host_tick(2 + step);
        next_events.extend(screenshot_events(&next));
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(next.owner(), first_slot);
    assert!(next_events.is_empty());
    assert!(!path.exists());
    assert!(!capturing());
}

#[test]
fn a_capture_only_pending_already_keeps_frames_off_the_idle_path() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    host.frame(0);
    let capturing_before = capturing();

    let sequence = request(client.handle(), &shot(&host, "a.png"));
    let capturing_pending = capturing();
    let offered = captures(&host.frame(1));

    assert!(!capturing_before);
    assert!(capturing_pending);
    assert_eq!(offered, vec![sequence]);
}

#[test]
fn a_capture_no_present_takes_fails_after_its_tick_limit() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let sequence = request(client.handle(), &shot(&host, "a.png"));
    let limit = u64::from(CaptureBook::PENDING_TICKS);

    for now in 1..limit {
        amm_host_tick(now);
    }
    let before_limit = screenshot_events(&client);
    amm_host_tick(limit);

    assert!(before_limit.is_empty());
    assert_eq!(screenshot_events(&client), vec![(AmmEventKind::ScreenshotTaken, sequence, 0)]);
    assert!(captures(&host.frame(limit + 1)).is_empty());
    assert!(!capturing());
}

#[test]
fn a_capture_with_layers_is_given_to_the_overlay_in_the_layer_set() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let path = shot(&host, "layers.png");
    let sequence = request_layers(client.handle(), &path, 1);

    let frame = host.frame(0);
    let (game, layered) = (captures(&frame), layer_captures(&frame));
    assert!(fill(sequence, 1, 1, &[30, 20, 10, 255]));
    assert_eq!(amm_host_capture_done(sequence, 1, BGRA), AmmStatus::Ok);
    let again = layer_captures(&host.frame(1));
    let events = wait_for_written(&client, 2);

    assert!(game.is_empty());
    assert_eq!(layered, vec![sequence]);
    assert!(again.is_empty());
    assert_eq!(
        events,
        vec![(AmmEventKind::ScreenshotTaken, sequence, 1), (AmmEventKind::ScreenshotWritten, sequence, 1)]
    );
    assert_eq!(&std::fs::read(&path).unwrap_or_default()[..4], b"\x89PNG");
}

#[test]
fn a_game_only_capture_stays_in_the_game_set() {
    let host = TestHost::new();
    let game_only = overlay_client(&host, "game-only");
    let by_value = overlay_client(&host, "by-value");
    let layered = overlay_client(&host, "layered");
    let first = request(game_only.handle(), &shot(&host, "a.png"));
    let second = request_layers(by_value.handle(), &shot(&host, "b.png"), 0);
    let third = request_layers(layered.handle(), &shot(&host, "c.png"), 1);

    let frame = host.frame(0);

    assert_eq!(captures(&frame), vec![first, second]);
    assert_eq!(layer_captures(&frame), vec![third]);
}
