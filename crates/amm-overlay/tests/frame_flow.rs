use std::sync::atomic::Ordering;
use std::time::Duration;

use amm_ffi::types::AmmHostPointerKind;
use amm_overlay::{Overlay, ShutdownOutcome, UninstallOutcome, WndProcRouter};
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::Graphics::Dxgi::DXGI_PRESENT_DO_NOT_WAIT;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, MSG, MoveWindow, PM_REMOVE, PeekMessageW, SendMessageW, WM_MOUSEMOVE, WM_SIZE,
};

use super::support::PixelCheck;
use super::support::d3d12_window::D3d12Window;
use super::support::log_capture::LogCapture;
use super::support::message_log::{MessageLog, Seen};
use super::support::retour_installer::RetourInstaller;
use super::support::swap_window::SwapWindow;
use super::support::test_host::TestHost;
use super::support::test_sink::TestSink;

const TIMEOUT: Duration = Duration::from_millis(2000);

#[test]
fn present_before_game_ready_binds_nothing() {
    let host = TestHost::leaked();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let first = SwapWindow::new();
    let second = SwapWindow::new();
    for _ in 0..3 {
        first.present();
    }
    assert_eq!(host.frames(), 0);
    host.set_ready();
    second.present();
    assert_eq!(host.frames(), 1, "the first chain after game_ready is bound");
    first.present();
    assert_eq!(host.frames(), 1, "the chain presented before game_ready was not bound");
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_red_layer_turns_the_back_buffer_red() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    let capture = host.capture_of(&window);
    assert_eq!(capture.sequence, TestHost::CAPTURE_SEQUENCE);
    assert!(capture.ok);
    let last = SwapWindow::SIZE - 1;
    for (x, y) in [(0, 0), (last / 2, last / 2), (last, last)] {
        let pixel = capture.pixel(x, y);
        assert!(PixelCheck::near(pixel, TestHost::RED, 1), "pixel ({x}, {y}) is {pixel:?}");
    }
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_chain_on_another_window_is_ignored() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let game = SwapWindow::new();
    let other = SwapWindow::new();
    game.present();
    assert_eq!(host.frames(), 1);
    other.present();
    other.present();
    assert_eq!(host.frames(), 1);
    game.present();
    assert_eq!(host.frames(), 2);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn resize_drops_the_textures_and_the_next_frame_has_the_new_size() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    assert_eq!(host.last_size(), (SwapWindow::SIZE, SwapWindow::SIZE));
    assert_eq!(host.textures_lost(), 0);
    // ResizeBuffers fails while any reference to a back buffer is left
    window.resize(128, 96).unwrap();
    window.present();
    assert_eq!(host.textures_lost(), 1);
    assert_eq!(host.last_size(), (128, 96));
    assert_eq!(Overlay::<RetourInstaller>::back_buffer_size(), (128, 96));
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_bind_and_a_resize_log_the_bound_size() {
    LogCapture::install();
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    let size = u64::from(SwapWindow::SIZE);
    assert_eq!(LogCapture::bound_sizes(), vec![(size, size)]);
    window.resize(128, 96).unwrap();
    window.present();
    assert_eq!(LogCapture::bound_sizes(), vec![(size, size), (128, 96)]);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn the_present_pace_logs_sync_changes_and_the_span_totals_when_the_forced_size_is_released() {
    const CHANGED: &str = "present sync changed";
    const SPAN: &str = "present pace while forced";
    LogCapture::install();
    let host = TestHost::leaked();
    host.set_ready();
    *host.forced_size.lock().unwrap() = Some((800, 450));
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    let changed = LogCapture::count(CHANGED);
    let spans = LogCapture::count(SPAN);
    // the first Present binds the chain and sees the size forced, with no ResizeBuffers on the way
    window.present();
    window.present();
    assert_eq!(LogCapture::count(CHANGED), changed + 1, "the first key is logged");
    window.present_with(DXGI_PRESENT_DO_NOT_WAIT);
    assert_eq!(LogCapture::count(SPAN), spans, "the span has not ended");

    *host.forced_size.lock().unwrap() = None;
    window.present();
    assert_eq!(LogCapture::count(SPAN), spans + 1, "the release ends the span");
    window.present_with(DXGI_PRESENT_DO_NOT_WAIT);
    window.present();
    assert_eq!(LogCapture::count(SPAN), spans + 1);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_sync_key_that_changes_on_every_present_logs_at_most_once_per_window() {
    const CHANGED: &str = "present sync changed";
    LogCapture::install();
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    let changed = LogCapture::count(CHANGED);
    for _ in 0..20 {
        window.present();
        window.present_with(DXGI_PRESENT_DO_NOT_WAIT);
    }
    let logged = LogCapture::count(CHANGED) - changed;
    // the 40 Presents take far less than a second, so one counting window; a second is allowed for a slow machine
    assert!((1..=2).contains(&logged), "{logged} lines for 40 key changes");
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_new_device_on_the_bound_window_rebuilds_the_overlay() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::bitblt();
    window.present();
    assert_eq!(host.frames(), 1);
    let window = window.rebuilt();
    let capture = host.capture_of(&window);
    assert_eq!(host.textures_lost(), 1);
    assert!(capture.ok);
    assert!(PixelCheck::near(capture.pixel(10, 10), TestHost::RED, 1));
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn the_d3d12_path_draws_through_the_game_queue_and_survives_a_resize() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = D3d12Window::new();
    window.present();
    assert_eq!(host.frames(), 0, "no frame before the game's queue is seen");
    window.submit();
    let capture = host.capture_with(|| window.present());
    assert!(capture.ok);
    let last = SwapWindow::SIZE - 1;
    for (x, y) in [(0, 0), (last, last)] {
        let pixel = capture.pixel(x, y);
        assert!(PixelCheck::near(pixel, TestHost::RED, 1), "pixel ({x}, {y}) is {pixel:?}");
    }
    let lost = host.textures_lost();
    // ResizeBuffers fails while a wrapper of a back buffer is left
    window.resize(128, 96).unwrap();
    window.present();
    assert_eq!(host.textures_lost(), lost + 1);
    assert_eq!(host.last_size(), (128, 96));
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn binding_the_game_window_routes_its_input_in_back_buffer_pixels() {
    let host = TestHost::leaked();
    let sink = TestSink::leaked();
    let overlay = Overlay::install(RetourInstaller::new(), host, Some(sink)).unwrap();
    let window = SwapWindow::new();
    window.present();
    // SAFETY: the window is live and belongs to this thread
    unsafe { SendMessageW(window.window.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(LPARAM(0x0010_0020))) };
    assert!(sink.pointers().is_empty(), "nothing is bound before the game is ready");
    host.set_ready();
    window.present();
    // SAFETY: as above
    unsafe { SendMessageW(window.window.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(LPARAM(0x0010_0020))) };
    let calls = sink.pointers();
    assert_eq!(calls.len(), 1);
    assert_eq!((calls[0].kind, calls[0].x, calls[0].y), (AmmHostPointerKind::Move, 32.0, 16.0));
    // the client area is twice the back buffer now: positions halve
    let size = SwapWindow::SIZE as i32 * 2;
    // SAFETY: the window is live
    unsafe { MoveWindow(window.window.hwnd, 0, 0, size, size, false) }.unwrap();
    // SAFETY: as above
    unsafe { SendMessageW(window.window.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(LPARAM(0x0010_0020))) };
    let calls = sink.pointers();
    assert_eq!((calls[1].x, calls[1].y), (16.0, 8.0));
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_device_rebuild_after_the_router_uninstall_does_not_subclass_the_window_again() {
    let host = TestHost::leaked();
    host.set_ready();
    let sink = TestSink::leaked();
    let overlay = Overlay::install(RetourInstaller::new(), host, Some(sink)).unwrap();
    let window = SwapWindow::bitblt();
    window.present();
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    let window = window.rebuilt();
    window.present();
    assert_eq!(host.textures_lost(), 1, "the new device was built");
    // SAFETY: the window is live and belongs to this thread
    unsafe { SendMessageW(window.window.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(LPARAM(0x0010_0020))) };
    assert!(sink.pointers().is_empty());
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::NotInstalled);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn the_next_overlay_install_subclasses_the_window_again() {
    let host = TestHost::leaked();
    host.set_ready();
    let sink = TestSink::leaked();
    let overlay = Overlay::install(RetourInstaller::new(), host, Some(sink)).unwrap();
    let window = SwapWindow::new();
    window.present();
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
    let overlay = Overlay::install(RetourInstaller::new(), host, Some(sink)).unwrap();
    window.present();
    // SAFETY: the window is live and belongs to this thread
    unsafe { SendMessageW(window.window.hwnd, WM_MOUSEMOVE, Some(WPARAM(0)), Some(LPARAM(0x0010_0020))) };
    assert_eq!(sink.pointers().len(), 1);
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_stream_frame_reaches_stream_done_and_not_capture_done() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    host.want_stream.store(true, Ordering::SeqCst);
    for _ in 0..TestHost::STREAM_PRESENTS {
        window.present();
        if !host.stream_reports().is_empty() {
            break;
        }
    }
    assert_eq!(host.stream_reports(), vec![(TestHost::STREAM_ID, true, false)]);
    assert!(host.take_capture().is_none());
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

// proves that a stream release loses no textures, that a screenshot in flight still completes, and that no stream
// report follows the release; WARP cannot hold a stream copy unread, so a release with a stream copy in flight is
// not reached here
#[test]
fn release_stream_drops_the_stream_pool_and_keeps_a_screenshot_in_flight() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    host.want_stream_and_capture.store(true, Ordering::SeqCst);
    window.present();
    host.want_release_stream.store(true, Ordering::SeqCst);
    let mut capture = None;
    for _ in 0..TestHost::STREAM_PRESENTS {
        window.present();
        capture = capture.or_else(|| host.take_capture());
        if capture.is_some() {
            break;
        }
    }
    let capture = capture.expect("the screenshot is reported");
    assert_eq!(capture.sequence, TestHost::GAME_CAPTURE_SEQUENCE);
    assert!(capture.ok);
    assert_eq!(host.textures_lost(), 0);
    // a report before the release frame is allowed; a good one after it is not
    assert!(host.stream_reports().iter().all(|(_, ok, released)| !(*ok && *released)));
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_resize_with_stream_copies_in_flight_signals_textures_lost_once() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    window.present();
    host.want_stream.store(true, Ordering::SeqCst);
    window.present();
    window.resize(128, 96).unwrap();
    window.present();
    assert_eq!(host.textures_lost(), 1);
    assert!(host.stream_reports().is_empty());
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}

#[test]
fn a_frame_size_message_posts_wm_size_to_the_bound_window() {
    let host = TestHost::leaked();
    host.set_ready();
    let overlay = Overlay::install(RetourInstaller::new(), host, None).unwrap();
    let window = SwapWindow::new();
    MessageLog::clear();
    MessageLog::install_bottom(window.window.hwnd);
    window.present();
    host.want_size_message.store(true, Ordering::SeqCst);
    window.present();
    window.present();
    let mut message = MSG::default();
    // SAFETY: the window belongs to this thread, and the message is a writable local
    while unsafe { PeekMessageW(&mut message, None, 0, 0, PM_REMOVE) }.as_bool() {
        // SAFETY: the message came from this thread's queue
        unsafe { DispatchMessageW(&message) };
    }
    let sizes: Vec<Seen> = MessageLog::bottom_seen().into_iter().filter(|seen| seen.message == WM_SIZE).collect();
    assert_eq!(sizes, vec![Seen { message: WM_SIZE, wparam: 0, lparam: (450 << 16) | 800 }]);
    assert_eq!(overlay.shutdown(TIMEOUT), ShutdownOutcome::Drained);
}
