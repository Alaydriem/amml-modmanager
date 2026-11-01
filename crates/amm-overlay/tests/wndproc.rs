use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use amm_abi::types::AmmCursorShape;
use amm_ffi::types::AmmHostPointerKind;
use amm_overlay::{PointerButton, UninstallOutcome, WndProcRouter};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, HTCLIENT, MSG, PostMessageW, PostThreadMessageW, SIZE_MINIMIZED, SIZE_RESTORED,
    SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SendMessageW, TranslateMessage, WINDOWPOS, WM_CHAR, WM_DISPLAYCHANGE,
    WM_DPICHANGED, WM_KILLFOCUS, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_QUIT, WM_SETCURSOR,
    WM_SIZE, WM_WINDOWPOSCHANGED,
};

use super::support::message_log::{MessageLog, Seen};
use super::support::test_sink::{PointerCall, TestSink};
use super::support::test_window::TestWindow;

const SIZE: u32 = 64;

struct Setup {
    window: TestWindow,
    sink: &'static TestSink,
}

impl Setup {
    // a window whose own procedure is the bottom recorder, with the router on top
    fn new() -> Self {
        let window = TestWindow::new(SIZE);
        MessageLog::clear();
        MessageLog::install_bottom(window.hwnd);
        let sink = TestSink::leaked();
        assert!(WndProcRouter::install(window.hwnd, sink));
        Self { window, sink }
    }

    fn send(&self, message: u32, wparam: usize, lparam: isize) -> isize {
        // SAFETY: the window is live and belongs to this thread
        unsafe { SendMessageW(self.window.hwnd, message, Some(WPARAM(wparam)), Some(LPARAM(lparam))) }.0
    }

    fn position(x: i16, y: i16) -> isize {
        (i32::from(y) << 16 | i32::from(x as u16)) as isize
    }

    fn wheel(delta: i16) -> usize {
        (delta as u16 as usize) << 16
    }

    // the router tells the sink once and the game's own procedure gets `message` with its own parameters
    fn assert_window_change_reaches_both(&self, message: u32, wparam: usize, lparam: isize) {
        self.send(message, wparam, lparam);
        assert_eq!(self.sink.window_changed_calls.load(Ordering::SeqCst), 1);
        let seen: Vec<Seen> = MessageLog::bottom_seen().into_iter().filter(|seen| seen.message == message).collect();
        assert_eq!(seen, vec![Seen { message, wparam, lparam }]);
    }
}

#[test]
fn a_consumed_button_press_returns_zero_and_the_game_never_sees_it() {
    let setup = Setup::new();
    setup.sink.consume.store(true, Ordering::SeqCst);
    let result = setup.send(WM_LBUTTONDOWN, 1, Setup::position(10, 20));
    assert_eq!(result, 0);
    assert!(MessageLog::bottom_seen().is_empty());
    assert!(WndProcRouter::consumed(PointerButton::Left));
    assert!(!WndProcRouter::consumed(PointerButton::Right));
    assert_eq!(
        setup.sink.pointers(),
        vec![PointerCall { kind: AmmHostPointerKind::Button, x: 10.0, y: 20.0, button: 0, down: true, wheel: 0.0 }]
    );
}

#[test]
fn a_release_the_sink_leaves_reaches_the_game_and_clears_the_consumed_slot() {
    let setup = Setup::new();
    setup.sink.consume.store(true, Ordering::SeqCst);
    setup.send(WM_LBUTTONDOWN, 1, Setup::position(1, 2));
    assert!(WndProcRouter::consumed(PointerButton::Left));
    setup.sink.consume.store(false, Ordering::SeqCst);
    setup.send(WM_LBUTTONUP, 0, Setup::position(1, 2));
    assert!(!WndProcRouter::consumed(PointerButton::Left));
    assert_eq!(
        MessageLog::bottom_seen(),
        vec![Seen { message: WM_LBUTTONUP, wparam: 0, lparam: Setup::position(1, 2) }]
    );
}

#[test]
fn a_move_the_sink_does_not_consume_reaches_the_game_unchanged() {
    let setup = Setup::new();
    let lparam = Setup::position(30, 40);
    setup.send(WM_MOUSEMOVE, 0x28, lparam);
    assert_eq!(MessageLog::bottom_seen(), vec![Seen { message: WM_MOUSEMOVE, wparam: 0x28, lparam }]);
    assert_eq!(
        setup.sink.pointers(),
        vec![PointerCall { kind: AmmHostPointerKind::Move, x: 30.0, y: 40.0, button: 0, down: false, wheel: 0.0 }]
    );
}

#[test]
fn a_position_left_of_the_window_keeps_its_sign() {
    let setup = Setup::new();
    setup.send(WM_MOUSEMOVE, 0, Setup::position(-10, -3));
    let call = setup.sink.pointers()[0];
    assert_eq!((call.x, call.y), (-10.0, -3.0));
}

#[test]
fn the_wheel_is_reported_in_notches() {
    let setup = Setup::new();
    setup.sink.consume.store(true, Ordering::SeqCst);
    setup.send(WM_MOUSEWHEEL, Setup::wheel(240), 0);
    setup.send(WM_MOUSEWHEEL, Setup::wheel(-120), 0);
    let wheels: Vec<f32> = setup.sink.pointers().iter().map(|call| call.wheel).collect();
    assert_eq!(wheels, vec![2.0, -1.0]);
    assert!(WndProcRouter::consumed(PointerButton::Wheel));
    assert!(MessageLog::bottom_seen().is_empty());
}

#[test]
fn a_surrogate_pair_gives_one_code_point_and_a_lone_half_is_dropped() {
    let setup = Setup::new();
    setup.send(WM_CHAR, 0xD83D, 0);
    setup.send(WM_CHAR, 0xDE00, 0);
    setup.send(WM_CHAR, 0xDE00, 0);
    setup.send(WM_CHAR, 0xD83D, 0);
    setup.send(WM_CHAR, 0x41, 0);
    setup.send(WM_CHAR, 0xE9, 0);
    assert_eq!(setup.sink.chars(), vec![0x1F600, 0x41, 0xE9]);
}

#[test]
fn losing_focus_tells_the_sink_once_and_the_game_still_sees_it() {
    let setup = Setup::new();
    setup.send(WM_KILLFOCUS, 0, 0);
    assert_eq!(setup.sink.focus_lost_calls.load(Ordering::SeqCst), 1);
    assert!(WndProcRouter::take_focus_lost());
    assert!(!WndProcRouter::take_focus_lost());
    assert_eq!(MessageLog::bottom_seen().len(), 1);
}

#[test]
fn a_free_cursor_is_set_over_the_client_area_and_otherwise_left_to_the_game() {
    let setup = Setup::new();
    *setup.sink.cursor.lock().unwrap() = (true, AmmCursorShape::Hand);
    assert_eq!(setup.send(WM_SETCURSOR, 0, HTCLIENT as isize), 1);
    assert!(MessageLog::bottom_seen().is_empty());
    // the border is not ours
    setup.send(WM_SETCURSOR, 0, 10);
    assert_eq!(MessageLog::bottom_seen().len(), 1);
    *setup.sink.cursor.lock().unwrap() = (false, AmmCursorShape::Hand);
    setup.send(WM_SETCURSOR, 0, HTCLIENT as isize);
    assert_eq!(MessageLog::bottom_seen().len(), 2);
}

#[test]
fn uninstall_on_top_restores_the_original_and_stops_routing() {
    let setup = Setup::new();
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    assert_eq!(MessageLog::current(setup.window.hwnd), MessageLog::bottom_address());
    setup.send(WM_MOUSEMOVE, 0, Setup::position(5, 5));
    assert!(setup.sink.pointers().is_empty());
    assert_eq!(MessageLog::bottom_seen().len(), 1);
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::NotInstalled);
}

#[test]
fn uninstall_under_a_second_subclass_keeps_the_chain_working() {
    let setup = Setup::new();
    MessageLog::install_top(setup.window.hwnd);
    let top = MessageLog::current(setup.window.hwnd);
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::PassThrough);
    assert_eq!(MessageLog::current(setup.window.hwnd), top, "the second subclass keeps its place");
    let lparam = Setup::position(7, 8);
    setup.send(WM_MOUSEMOVE, 0, lparam);
    assert_eq!(MessageLog::top_seen(), vec![Seen { message: WM_MOUSEMOVE, wparam: 0, lparam }]);
    assert_eq!(MessageLog::bottom_seen(), vec![Seen { message: WM_MOUSEMOVE, wparam: 0, lparam }]);
    assert!(setup.sink.pointers().is_empty(), "a router that is off does not reach the sink");
}

#[test]
fn uninstall_run_from_inside_a_sink_call_does_not_wait_for_itself() {
    let setup = Setup::new();
    setup.sink.uninstall_in_pointer.store(true, Ordering::SeqCst);
    setup.send(WM_MOUSEMOVE, 0, Setup::position(1, 1));
    let (outcome, took) = setup.sink.uninstall_outcome.lock().unwrap().take().unwrap();
    assert_eq!(outcome, UninstallOutcome::Restored);
    assert!(took < WndProcRouter::DRAIN_TIMEOUT / 2, "the drain waited {took:?}");
}

#[test]
fn a_panic_in_the_sink_stops_routing_and_the_message_still_reaches_the_game() {
    let setup = Setup::new();
    setup.sink.panic_on_pointer.store(true, Ordering::SeqCst);
    let quiet = super::support::quiet_panics::QuietPanics::new();
    setup.send(WM_MOUSEMOVE, 0, Setup::position(1, 1));
    drop(quiet);
    assert_eq!(MessageLog::bottom_seen().len(), 1);
    setup.send(WM_MOUSEMOVE, 0, Setup::position(2, 2));
    assert_eq!(setup.sink.pointers().len(), 1, "the sink was not called again");
    assert_eq!(MessageLog::bottom_seen().len(), 2);
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
}

// a window with its own message loop thread, so a sink call can block there while another thread uninstalls
#[test]
fn uninstall_stops_waiting_when_another_thread_stays_inside_the_sink() {
    let sink = TestSink::leaked();
    let (sender, receiver) = mpsc::channel();
    let window_thread = std::thread::spawn(move || {
        let window = TestWindow::new(SIZE);
        assert!(WndProcRouter::install(window.hwnd, sink));
        // SAFETY: no input
        let thread_id = unsafe { GetCurrentThreadId() };
        sender.send((window.hwnd.0 as usize, thread_id)).unwrap();
        let mut message = MSG::default();
        // SAFETY: the out pointer is a writable local; the loop runs on the thread that made the window
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            // SAFETY: a message the loop just got
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drop(window);
    });
    let (hwnd, thread_id) = receiver.recv().unwrap();
    let hwnd = HWND(hwnd as *mut _);
    sink.block_pointer.store(true, Ordering::SeqCst);
    // SAFETY: the window is live until the quit below
    unsafe { PostMessageW(Some(hwnd), WM_MOUSEMOVE, WPARAM(0), LPARAM(0)) }.unwrap();
    while !sink.entered.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(1));
    }
    let start = Instant::now();
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::TimedOut);
    let took = start.elapsed();
    assert!(took >= WndProcRouter::DRAIN_TIMEOUT - Duration::from_millis(100), "returned after {took:?}");
    assert!(took < WndProcRouter::DRAIN_TIMEOUT * 3, "returned after {took:?}");
    sink.release.store(true, Ordering::SeqCst);
    // SAFETY: the id is of the window thread, which has a message queue
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.unwrap();
    window_thread.join().unwrap();
}

#[test]
fn wm_size_takes_the_size_the_sink_gives() {
    let setup = Setup::new();
    *setup.sink.window_size.lock().unwrap() = Some((800, 450));
    setup.send(WM_SIZE, SIZE_RESTORED as usize, Setup::position(64, 64));
    assert_eq!(
        MessageLog::bottom_seen(),
        vec![Seen { message: WM_SIZE, wparam: SIZE_RESTORED as usize, lparam: (450 << 16) | 800 }]
    );
    assert_eq!(*setup.sink.window_sizes.lock().unwrap(), vec![(false, 64, 64)]);
}

#[test]
fn a_minimised_wm_size_reaches_the_sink_as_minimised() {
    let setup = Setup::new();
    let lparam = Setup::position(0, 0);
    setup.send(WM_SIZE, SIZE_MINIMIZED as usize, lparam);
    assert_eq!(*setup.sink.window_sizes.lock().unwrap(), vec![(true, 0, 0)]);
    assert_eq!(MessageLog::bottom_seen(), vec![Seen { message: WM_SIZE, wparam: SIZE_MINIMIZED as usize, lparam }]);
}

// the game's own handler of `WM_SIZE` can run long, so it must not count as a thread inside the sink
#[test]
fn uninstall_does_not_wait_for_the_game_to_handle_wm_size() {
    let sink = TestSink::leaked();
    let (sender, receiver) = mpsc::channel();
    let window_thread = std::thread::spawn(move || {
        let window = TestWindow::new(SIZE);
        MessageLog::clear();
        MessageLog::install_bottom(window.hwnd);
        assert!(WndProcRouter::install(window.hwnd, sink));
        // SAFETY: no input
        let thread_id = unsafe { GetCurrentThreadId() };
        sender.send((window.hwnd.0 as usize, thread_id)).unwrap();
        let mut message = MSG::default();
        // SAFETY: the out pointer is a writable local; the loop runs on the thread that made the window
        while unsafe { GetMessageW(&mut message, None, 0, 0) }.as_bool() {
            // SAFETY: a message the loop just got
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        drop(window);
    });
    let (hwnd, thread_id) = receiver.recv().unwrap();
    let hwnd = HWND(hwnd as *mut _);
    *sink.window_size.lock().unwrap() = Some((800, 450));
    MessageLog::block_size();
    // SAFETY: the window is live until the quit below
    unsafe { PostMessageW(Some(hwnd), WM_SIZE, WPARAM(SIZE_RESTORED as usize), LPARAM(0)) }.unwrap();
    let wait = Instant::now();
    while !MessageLog::inside_size() {
        if wait.elapsed() > Duration::from_secs(2) {
            MessageLog::release_size();
            // SAFETY: the id is of the window thread, which has a message queue
            unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.unwrap();
            window_thread.join().unwrap();
            panic!("the window procedure never entered the held WM_SIZE");
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    let start = Instant::now();
    let outcome = WndProcRouter::uninstall();
    let took = start.elapsed();
    MessageLog::release_size();
    // SAFETY: the id is of the window thread, which has a message queue
    unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)) }.unwrap();
    window_thread.join().unwrap();
    assert_eq!(outcome, UninstallOutcome::Restored);
    assert!(took < Duration::from_millis(500), "uninstall waited {took:?}");
}

#[test]
fn a_dpi_change_tells_the_sink_and_reaches_the_game_unchanged() {
    let setup = Setup::new();
    let suggested = RECT { left: 0, top: 0, right: SIZE as i32, bottom: SIZE as i32 };
    let lparam = &suggested as *const RECT as isize;
    setup.assert_window_change_reaches_both(WM_DPICHANGED, (144 << 16) | 144, lparam);
}

#[test]
fn a_display_change_tells_the_sink_and_reaches_the_game_unchanged() {
    let setup = Setup::new();
    setup.assert_window_change_reaches_both(WM_DISPLAYCHANGE, 32, Setup::position(2560, 1440));
}

#[test]
fn a_window_position_change_tells_the_sink_and_reaches_the_game_unchanged() {
    let setup = Setup::new();
    let position =
        WINDOWPOS { hwnd: setup.window.hwnd, flags: SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER, ..WINDOWPOS::default() };
    let lparam = &position as *const WINDOWPOS as isize;
    setup.assert_window_change_reaches_both(WM_WINDOWPOSCHANGED, 0, lparam);
}

#[test]
fn a_window_change_after_uninstall_does_not_reach_the_sink() {
    let setup = Setup::new();
    assert_eq!(WndProcRouter::uninstall(), UninstallOutcome::Restored);
    setup.send(WM_DISPLAYCHANGE, 32, Setup::position(2560, 1440));
    assert_eq!(setup.sink.window_changed_calls.load(Ordering::SeqCst), 0);
}
