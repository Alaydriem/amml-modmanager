use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use windows::Win32::UI::WindowsAndMessaging::WNDPROC;

use crate::in_flight::InFlight;
use crate::input_sink::InputSink;
use crate::pointer_button::PointerButton;
use crate::surrogate_join::SurrogateJoin;

static STATE: RouterState = RouterState::new();

/// The process-wide state of `WndProcRouter`: a window procedure has no context pointer. The window lock guards
/// install and uninstall; the window procedure reads only the atomics.
pub(crate) struct RouterState {
    // the subclassed window's handle value, 0 when none; held across install and uninstall
    window: Mutex<usize>,
    // the procedure the window had before ours, as an address; 0 when none
    original: AtomicUsize,
    // routing is on
    forwarding: AtomicBool,
    // set by uninstall: the overlay's own bind of a window may not subclass it again until the next overlay
    // install opens it
    closed: AtomicBool,
    // a sink call panicked; routing stays off until the next install
    disabled: AtomicBool,
    panic_logged: AtomicBool,
    thread_logged: AtomicBool,
    // points to a leaked `&'static dyn InputSink`, so a fat pointer fits in one atomic; null until the first install
    sink: AtomicPtr<&'static dyn InputSink>,
    consumed: [AtomicBool; PointerButton::COUNT],
    // set by the window thread on focus loss and taken by a reader on another thread: the release store pairs with
    // the acquire swap in `take_focus_lost`
    focus_lost: AtomicBool,
    pub(crate) calls: InFlight,
    pub(crate) surrogates: SurrogateJoin,
}

impl RouterState {
    const fn new() -> Self {
        Self {
            window: Mutex::new(0),
            original: AtomicUsize::new(0),
            forwarding: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            disabled: AtomicBool::new(false),
            panic_logged: AtomicBool::new(false),
            thread_logged: AtomicBool::new(false),
            sink: AtomicPtr::new(std::ptr::null_mut()),
            consumed: [const { AtomicBool::new(false) }; PointerButton::COUNT],
            focus_lost: AtomicBool::new(false),
            calls: InFlight::new(),
            surrogates: SurrogateJoin::new(),
        }
    }

    pub(crate) fn global() -> &'static Self {
        &STATE
    }

    pub(crate) fn lock_window(&self) -> MutexGuard<'_, usize> {
        self.window.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn original(&self) -> WNDPROC {
        let address = self.original.load(Ordering::SeqCst);
        // SAFETY: the address is 0 or the procedure `GetWindowLongPtrW` gave for the window, stored by install;
        // 0 becomes `None`, as for any `Option` of a function pointer
        unsafe { std::mem::transmute::<usize, WNDPROC>(address) }
    }

    pub(crate) fn set_original(&self, address: usize) {
        self.original.store(address, Ordering::SeqCst);
    }

    pub(crate) fn set_forwarding(&self, on: bool) {
        self.forwarding.store(on, Ordering::SeqCst);
    }

    pub(crate) fn set_closed(&self, closed: bool) {
        self.closed.store(closed, Ordering::SeqCst);
    }

    pub(crate) fn closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Routing is on and no sink call has panicked.
    pub(crate) fn active(&self) -> bool {
        self.forwarding.load(Ordering::SeqCst) && !self.disabled.load(Ordering::SeqCst)
    }

    /// Turns routing back on after a panic, for a new install.
    pub(crate) fn rearm(&self) {
        self.disabled.store(false, Ordering::SeqCst);
        self.panic_logged.store(false, Ordering::SeqCst);
    }

    /// Stops routing after a panic. True the first time only, for the one log line.
    pub(crate) fn disable(&self) -> bool {
        self.disabled.store(true, Ordering::SeqCst);
        !self.panic_logged.swap(true, Ordering::SeqCst)
    }

    /// True the first time only.
    pub(crate) fn first_window_thread_call(&self) -> bool {
        !self.thread_logged.load(Ordering::Relaxed) && !self.thread_logged.swap(true, Ordering::Relaxed)
    }

    /// Keeps `sink` for the window procedure. The same sink again leaks nothing.
    pub(crate) fn set_sink(&self, sink: &'static dyn InputSink) {
        if self.sink().is_some_and(|current| std::ptr::addr_eq(current, sink)) {
            return;
        }
        let cell = Box::into_raw(Box::new(sink));
        self.sink.store(cell, Ordering::SeqCst);
    }

    pub(crate) fn sink(&self) -> Option<&'static dyn InputSink> {
        let cell = self.sink.load(Ordering::SeqCst);
        // SAFETY: a non-null cell comes from `set_sink`, which leaks it, so it lives for the process
        unsafe { cell.as_ref() }.copied()
    }

    pub(crate) fn set_consumed(&self, button: PointerButton, consumed: bool) {
        self.consumed[button as usize].store(consumed, Ordering::Relaxed);
    }

    pub(crate) fn consumed(&self, button: PointerButton) -> bool {
        self.consumed[button as usize].load(Ordering::Relaxed)
    }

    pub(crate) fn clear_consumed(&self) {
        for slot in &self.consumed {
            slot.store(false, Ordering::Relaxed);
        }
    }

    pub(crate) fn note_focus_lost(&self) {
        self.focus_lost.store(true, Ordering::Release);
    }

    pub(crate) fn take_focus_lost(&self) -> bool {
        self.focus_lost.swap(false, Ordering::Acquire)
    }
}
