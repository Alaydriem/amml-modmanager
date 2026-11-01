use std::panic::AssertUnwindSafe;
use std::time::Duration;

use amm_abi::types::AmmCursorShape;
use amm_ffi::types::AmmHostPointerKind;
use windows::Win32::Foundation::{GetLastError, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, GetClientRect, GetWindowLongPtrW, HTCLIENT, IDC_ARROW, IDC_HAND,
    IDC_IBEAM, IsWindow, LoadCursorW, SIZE_MINIMIZED, SetCursor, SetWindowLongPtrW, WHEEL_DELTA, WM_CHAR,
    WM_DISPLAYCHANGE, WM_DPICHANGED, WM_KILLFOCUS, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDBLCLK,
    WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP,
    WM_SETCURSOR, WM_SIZE, WM_WINDOWPOSCHANGED,
};

use crate::call_scope::CallScope;
use crate::input_sink::InputSink;
use crate::overlay_shared::OverlayShared;
use crate::pointer_button::PointerButton;
use crate::router_state::RouterState;
use crate::size_message::SizeMessage;
use crate::uninstall_outcome::UninstallOutcome;

/// The one window procedure subclass on the game window. It sends pointer moves, buttons, the wheel, `WM_CHAR` and
/// focus loss to an `InputSink` in back-buffer pixels, swallows each message the sink consumes, and answers
/// `WM_SETCURSOR` while the sink says the cursor is free. Keys do not come through it.
pub struct WndProcRouter;

impl WndProcRouter {
    /// How long `uninstall` waits for a thread inside the sink.
    pub const DRAIN_TIMEOUT: Duration = Duration::from_millis(2000);

    /// Subclasses `hwnd` once and routes its input to `sink`. Installing again on the window it already has only
    /// switches routing back on. False when `hwnd` is null, the window is another one than the subclassed window,
    /// or Windows refused the subclass; the window then gets no layer input.
    pub fn install(hwnd: HWND, sink: &'static dyn InputSink) -> bool {
        Self::install_with(hwnd, sink, false)
    }

    /// `install` for the overlay's bind of the game window. Refused after `uninstall` until the next
    /// `Overlay::install` opens the router again, so a device rebuild during shutdown cannot subclass the window.
    pub(crate) fn install_bound(hwnd: HWND, sink: &'static dyn InputSink) -> bool {
        Self::install_with(hwnd, sink, true)
    }

    /// Lets `install_bound` subclass again.
    pub(crate) fn open() {
        let state = RouterState::global();
        let _window = state.lock_window();
        state.set_closed(false);
    }

    fn install_with(hwnd: HWND, sink: &'static dyn InputSink, bound: bool) -> bool {
        let state = RouterState::global();
        let mut window = state.lock_window();
        if hwnd.0.is_null() || (bound && state.closed()) {
            return false;
        }
        let address = hwnd.0 as usize;
        if *window != 0 {
            if *window != address {
                return false;
            }
            state.set_sink(sink);
            state.rearm();
            state.set_forwarding(true);
            return true;
        }
        state.set_sink(sink);
        state.rearm();
        // a message that arrives between the swap and the store below still finds the procedure the window had
        // SAFETY: the handle is a window, and only reads the procedure
        let current = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
        state.set_original(current as usize);
        // SAFETY: `window_proc` has the signature of a window procedure and lives for the process
        let previous = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, Self::address() as isize) };
        if previous == 0 {
            // SAFETY: no input
            let error = unsafe { GetLastError() }.0;
            curia::warn!("could not subclass the game window, layers get no mouse input", { error: error });
            state.set_original(0);
            return false;
        }
        state.set_original(previous as usize);
        *window = address;
        state.set_forwarding(true);
        curia::info!("game window subclassed");
        true
    }

    /// Stops routing, puts the original window procedure back when ours is still on top, and waits up to
    /// `DRAIN_TIMEOUT` until no other thread is inside the sink. The calling thread's own calls are not counted,
    /// so it may run from inside a sink call. Safe to call more than once. The overlay's binds do not subclass
    /// the window again until the next `Overlay::install`.
    pub fn uninstall() -> UninstallOutcome {
        let state = RouterState::global();
        let mut window = state.lock_window();
        state.set_closed(true);
        state.set_forwarding(false);
        state.clear_consumed();
        let mut outcome = UninstallOutcome::NotInstalled;
        if *window != 0 {
            if Self::restore_original(state, *window) {
                *window = 0;
                outcome = UninstallOutcome::Restored;
            } else {
                curia::warn!(
                    "another window procedure sits above the manager's, the manager's stays and only passes messages on"
                );
                outcome = UninstallOutcome::PassThrough;
            }
        }
        if state.calls.wait_until_at_most(CallScope::depth(), Self::DRAIN_TIMEOUT) {
            return outcome;
        }
        curia::warn!("a thread is still inside the input sink after the input drain");
        UninstallOutcome::TimedOut
    }

    /// True when the sink consumed the last press or release of `button` that came through the window, so the
    /// game's own copy of that event is cancelled too. Read on any thread.
    pub fn consumed(button: PointerButton) -> bool {
        RouterState::global().consumed(button)
    }

    /// True once after the game window lost focus, so a reader that tracks held keys forgets them. Any thread.
    pub fn take_focus_lost() -> bool {
        RouterState::global().take_focus_lost()
    }

    fn address() -> usize {
        Self::window_proc as *const () as usize
    }

    // contains a panic of the sink: it is logged once, routing stops, and the message goes to the original
    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        let state = RouterState::global();
        let routed = std::panic::catch_unwind(AssertUnwindSafe(|| Self::route(state, hwnd, message, wparam, lparam)));
        match routed {
            Ok(Some(result)) => result,
            Ok(None) => Self::pass_on(state, hwnd, message, wparam, lparam),
            Err(_) => {
                if state.disable() {
                    curia::error!("game window input routing panicked, the layers get no input for this session");
                }
                Self::pass_on(state, hwnd, message, wparam, lparam)
            }
        }
    }

    // the result to return when the manager handled the message, `None` to let the original see it
    fn route(state: &RouterState, hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        if state.first_window_thread_call() {
            // SAFETY: no input
            let thread = unsafe { GetCurrentThreadId() };
            curia::info!("game window procedure runs on its thread", { thread: thread });
        }
        if !state.active() {
            return None;
        }
        if message == WM_SETCURSOR && (lparam.0 & 0xFFFF) as u32 == HTCLIENT {
            return Self::set_cursor(state);
        }
        if message == WM_SIZE {
            return Self::window_size(state, hwnd, wparam, lparam);
        }
        if matches!(message, WM_DPICHANGED | WM_DISPLAYCHANGE | WM_WINDOWPOSCHANGED) {
            Self::window_changed(state);
            return None;
        }
        if !Self::is_forwarded(message) {
            return None;
        }
        // counted for the drain; routing is checked again inside the count, so no call starts after uninstall saw 0
        let _scope = CallScope::enter(&state.calls);
        if !state.active() {
            return None;
        }
        let sink = state.sink()?;
        Self::forward(state, sink, hwnd, message, wparam, lparam).then_some(LRESULT(0))
    }

    // the game gets the size the sink gives in place of the one the message carries
    fn window_size(state: &RouterState, hwnd: HWND, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        // the count ends before the game's own procedure runs, so a long handler there never holds up the drain
        let (width, height) = {
            let _scope = CallScope::enter(&state.calls);
            if !state.active() {
                return None;
            }
            let sink = state.sink()?;
            let width = (lparam.0 & 0xFFFF) as u32;
            let height = ((lparam.0 >> 16) & 0xFFFF) as u32;
            sink.window_size(wparam.0 == SIZE_MINIMIZED as usize, width, height)?
        };
        Some(Self::pass_on(state, hwnd, WM_SIZE, wparam, SizeMessage::size_param(width, height)))
    }

    // the sink hears of the change; the game's own procedure gets the message unchanged
    fn window_changed(state: &RouterState) {
        let _scope = CallScope::enter(&state.calls);
        if !state.active() {
            return;
        }
        if let Some(sink) = state.sink() {
            sink.window_changed();
        }
    }

    fn set_cursor(state: &RouterState) -> Option<LRESULT> {
        let _scope = CallScope::enter(&state.calls);
        if !state.active() {
            return None;
        }
        let (free, shape) = state.sink()?.cursor();
        if !free {
            return None;
        }
        let id = match shape {
            AmmCursorShape::Hand => IDC_HAND,
            AmmCursorShape::Text => IDC_IBEAM,
            AmmCursorShape::Default | AmmCursorShape::Arrow => IDC_ARROW,
        };
        // SAFETY: a system cursor is shared and never freed
        let cursor = unsafe { LoadCursorW(None, id) }.ok()?;
        // SAFETY: the cursor is live
        unsafe { SetCursor(Some(cursor)) };
        Some(LRESULT(1))
    }

    fn is_forwarded(message: u32) -> bool {
        matches!(
            message,
            WM_MOUSEMOVE
                | WM_LBUTTONDOWN
                | WM_LBUTTONDBLCLK
                | WM_LBUTTONUP
                | WM_RBUTTONDOWN
                | WM_RBUTTONDBLCLK
                | WM_RBUTTONUP
                | WM_MBUTTONDOWN
                | WM_MBUTTONDBLCLK
                | WM_MBUTTONUP
                | WM_MOUSEWHEEL
                | WM_CHAR
                | WM_KILLFOCUS
        )
    }

    // true when the sink consumed the message
    fn forward(
        state: &RouterState,
        sink: &dyn InputSink,
        hwnd: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> bool {
        match message {
            WM_MOUSEMOVE => Self::pointer(sink, hwnd, AmmHostPointerKind::Move, lparam, 0, false),
            WM_LBUTTONDOWN | WM_LBUTTONDBLCLK => Self::button(state, sink, hwnd, lparam, PointerButton::Left, true),
            WM_LBUTTONUP => Self::button(state, sink, hwnd, lparam, PointerButton::Left, false),
            WM_RBUTTONDOWN | WM_RBUTTONDBLCLK => Self::button(state, sink, hwnd, lparam, PointerButton::Right, true),
            WM_RBUTTONUP => Self::button(state, sink, hwnd, lparam, PointerButton::Right, false),
            WM_MBUTTONDOWN | WM_MBUTTONDBLCLK => Self::button(state, sink, hwnd, lparam, PointerButton::Middle, true),
            WM_MBUTTONUP => Self::button(state, sink, hwnd, lparam, PointerButton::Middle, false),
            WM_MOUSEWHEEL => Self::wheel(state, sink, wparam),
            WM_CHAR => state.surrogates.feed(wparam.0 as u32).is_some_and(|codepoint| sink.char(codepoint)),
            WM_KILLFOCUS => {
                state.surrogates.reset();
                state.note_focus_lost();
                sink.focus_lost();
                false
            }
            _ => false,
        }
    }

    fn pointer(
        sink: &dyn InputSink,
        hwnd: HWND,
        kind: AmmHostPointerKind,
        lparam: LPARAM,
        button: u8,
        down: bool,
    ) -> bool {
        // the low and high words are signed: a window can be left of or above the cursor
        let x = f32::from(lparam.0 as i16);
        let y = f32::from((lparam.0 >> 16) as i16);
        let (x, y) = Self::to_back_buffer(hwnd, x, y);
        sink.pointer(kind, x, y, button, down, 0.0)
    }

    fn button(
        state: &RouterState,
        sink: &dyn InputSink,
        hwnd: HWND,
        lparam: LPARAM,
        button: PointerButton,
        down: bool,
    ) -> bool {
        let consume = Self::pointer(sink, hwnd, AmmHostPointerKind::Button, lparam, button as u8, down);
        state.set_consumed(button, consume);
        consume
    }

    fn wheel(state: &RouterState, sink: &dyn InputSink, wparam: WPARAM) -> bool {
        let notches = f32::from((wparam.0 >> 16) as i16) / WHEEL_DELTA as f32;
        let consume = sink.pointer(AmmHostPointerKind::Wheel, 0.0, 0.0, 0, false, notches);
        state.set_consumed(PointerButton::Wheel, consume);
        consume
    }

    // window messages give client-area pixels and the layers lay out in back-buffer pixels; they differ when the
    // game renders at another size than its client area (another fullscreen mode, or the frames before a resize)
    fn to_back_buffer(hwnd: HWND, x: f32, y: f32) -> (f32, f32) {
        let (width, height) = OverlayShared::global().back_buffer_size();
        let mut client = RECT::default();
        // SAFETY: the out pointer is a writable local
        if width == 0 || height == 0 || unsafe { GetClientRect(hwnd, &mut client) }.is_err() {
            return (x, y);
        }
        if client.right <= 0 || client.bottom <= 0 {
            return (x, y);
        }
        (x * width as f32 / client.right as f32, y * height as f32 / client.bottom as f32)
    }

    fn pass_on(state: &RouterState, hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        match state.original() {
            // SAFETY: the original is the window's procedure from before ours, called with the message we got
            Some(_) => unsafe { CallWindowProcW(state.original(), hwnd, message, wparam, lparam) },
            // SAFETY: the arguments of a live message
            None => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
        }
    }

    // another subclass set after ours would lose its place if ours were restored under it
    fn restore_original(state: &RouterState, window: usize) -> bool {
        let hwnd = HWND(window as *mut _);
        // SAFETY: a handle that is not a window gives false
        if !unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            return true;
        }
        // SAFETY: the handle is a window
        let current = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
        if current as usize != Self::address() {
            return false;
        }
        // SAFETY: the stored address is the procedure the window had before ours
        unsafe {
            SetWindowLongPtrW(hwnd, GWLP_WNDPROC, state.original().map_or(0, |original| original as usize as isize))
        };
        true
    }
}
