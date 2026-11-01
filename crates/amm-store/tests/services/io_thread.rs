use std::time::Duration;

use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, CaptureOutcome, PixelFormat};
use amm_core::structs::write::WriteRequest;
use amm_store::services::{IoQueue, IoThread};

#[test]
fn shutdown_flushes_pending_writes() {
    let dir = tempfile::tempdir().unwrap();
    let thread = IoThread::new(IoQueue::new(32)).unwrap();
    for index in 0..20 {
        let path = dir.path().join(format!("mod{index}")).join("config.hcl");
        thread.submit(WriteRequest { path, text: format!("n = {index}\n") }).unwrap();
    }

    thread.shutdown(Duration::from_secs(5));

    for index in 0..20 {
        let path = dir.path().join(format!("mod{index}")).join("config.hcl");
        assert_eq!(std::fs::read_to_string(path).unwrap(), format!("n = {index}\n"));
    }
}

#[test]
fn submit_after_shutdown_is_unavailable() {
    let dir = tempfile::tempdir().unwrap();
    let queue = IoQueue::new(4);
    let thread = IoThread::new(queue.clone()).unwrap();

    thread.shutdown(Duration::from_secs(5));
    let result = queue.try_submit(WriteRequest { path: dir.path().join("late.hcl"), text: String::new() });

    assert_eq!(result, Err(StoreError::Unavailable));
    assert!(!dir.path().join("late.hcl").exists());
}

#[test]
fn a_failed_write_does_not_stop_later_writes() {
    let dir = tempfile::tempdir().unwrap();
    let blocker = dir.path().join("blocker");
    std::fs::write(&blocker, "file").unwrap();
    let thread = IoThread::new(IoQueue::new(4)).unwrap();
    thread.submit(WriteRequest { path: blocker.join("config.hcl"), text: "x".into() }).unwrap();
    thread.submit(WriteRequest { path: dir.path().join("ok.hcl"), text: "ok = 1\n".into() }).unwrap();

    thread.shutdown(Duration::from_secs(5));

    assert_eq!(std::fs::read_to_string(dir.path().join("ok.hcl")).unwrap(), "ok = 1\n");
}

fn capture(sequence: u32, path: std::path::PathBuf, format: u32) -> CaptureImage {
    CaptureImage { sequence, path, pixels: vec![255; 16], width: 2, height: 2, row_pitch: 8, format }
}

fn wait_for_outcome(thread: &IoThread) -> Option<CaptureOutcome> {
    for _ in 0..500 {
        if let Some(outcome) = thread.written_captures().next() {
            return Some(outcome);
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    None
}

#[test]
fn a_capture_job_writes_its_png_and_reports_back() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");
    let thread = IoThread::new(IoQueue::new(4)).unwrap();

    thread.submit_capture(capture(7, path.clone(), PixelFormat::DXGI_R8G8B8A8_UNORM)).unwrap();
    let outcome = wait_for_outcome(&thread);

    assert_eq!(outcome, Some(CaptureOutcome { sequence: 7, ok: true }));
    assert!(path.is_file());
    thread.shutdown(Duration::from_secs(5));
}

#[test]
fn a_capture_in_a_refused_format_reports_a_failure_and_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");
    let thread = IoThread::new(IoQueue::new(4)).unwrap();

    thread.submit_capture(capture(9, path.clone(), 10)).unwrap();
    let outcome = wait_for_outcome(&thread);

    assert_eq!(outcome, Some(CaptureOutcome { sequence: 9, ok: false }));
    assert!(!path.exists());
    thread.shutdown(Duration::from_secs(5));
}

#[test]
fn captures_queued_before_a_config_write_at_shutdown_fail_and_the_config_write_lands() {
    let dir = tempfile::tempdir().unwrap();
    let queue = IoQueue::new(8);
    let shots: Vec<_> = (1..=4).map(|n| dir.path().join(format!("shot{n}.png"))).collect();
    for (n, shot) in (1..).zip(&shots) {
        queue.try_submit_capture(capture(n, shot.clone(), PixelFormat::DXGI_R8G8B8A8_UNORM)).unwrap();
    }
    let config = dir.path().join("config.hcl");
    queue.try_submit(WriteRequest { path: config.clone(), text: "n = 1\n".into() }).unwrap();
    queue.close();

    let thread = IoThread::new(queue).unwrap();
    let outcomes: Vec<_> = (0..4).filter_map(|_| wait_for_outcome(&thread)).collect();
    thread.shutdown(Duration::from_secs(2));

    assert_eq!(std::fs::read_to_string(config).unwrap(), "n = 1\n");
    let failed: Vec<_> = (1..=4).map(|sequence| CaptureOutcome { sequence, ok: false }).collect();
    assert_eq!(outcomes, failed);
    assert!(shots.iter().all(|shot| !shot.exists()));
}
