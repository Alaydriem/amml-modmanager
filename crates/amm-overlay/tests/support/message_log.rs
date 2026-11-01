use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, DefWindowProcW, GWLP_WNDPROC, GetWindowLongPtrW, SetWindowLongPtrW, WM_SIZE, WNDPROC,
};

/// One message a window procedure of a test saw.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seen {
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
}

static BOTTOM: Mutex<Vec<Seen>> = Mutex::new(Vec::new());
static TOP: Mutex<Vec<Seen>> = Mutex::new(Vec::new());
static TOP_PREVIOUS: AtomicUsize = AtomicUsize::new(0);
static BLOCK_SIZE: AtomicBool = AtomicBool::new(false);
static IN_SIZE: AtomicBool = AtomicBool::new(false);

/// The window procedures a test puts on a window: a recorder at the bottom, which plays the game's own procedure,
/// and a recorder on top that passes on, which plays another mod's subclass.
pub struct MessageLog;

impl MessageLog {
    /// Makes the recorder the window's procedure, under whatever the test installs next.
    pub fn install_bottom(hwnd: HWND) {
        // SAFETY: `bottom` has the signature of a window procedure and lives for the process
        unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, Self::bottom as *const () as isize) };
    }

    /// Sets a procedure above the current one that records and passes on.
    pub fn install_top(hwnd: HWND) {
        // SAFETY: the handle is a window
        let previous = unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) };
        TOP_PREVIOUS.store(previous as usize, Ordering::SeqCst);
        // SAFETY: `top` has the signature of a window procedure and lives for the process
        unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, Self::top as *const () as isize) };
    }

    /// The window's current procedure, as an address.
    pub fn current(hwnd: HWND) -> usize {
        // SAFETY: the handle is a window
        unsafe { GetWindowLongPtrW(hwnd, GWLP_WNDPROC) as usize }
    }

    pub fn bottom_address() -> usize {
        Self::bottom as *const () as usize
    }

    /// What reached the bottom recorder, oldest first.
    pub fn bottom_seen() -> Vec<Seen> {
        BOTTOM.lock().unwrap().clone()
    }

    /// What reached the top recorder, oldest first.
    pub fn top_seen() -> Vec<Seen> {
        TOP.lock().unwrap().clone()
    }

    /// Makes the bottom recorder stay inside its next `WM_SIZE` until `release_size` runs.
    pub fn block_size() {
        IN_SIZE.store(false, Ordering::SeqCst);
        BLOCK_SIZE.store(true, Ordering::SeqCst);
    }

    /// True while the bottom recorder is held inside a `WM_SIZE`.
    pub fn inside_size() -> bool {
        IN_SIZE.load(Ordering::SeqCst)
    }

    pub fn release_size() {
        BLOCK_SIZE.store(false, Ordering::SeqCst);
    }

    pub fn clear() {
        BOTTOM.lock().unwrap().clear();
        TOP.lock().unwrap().clear();
    }

    unsafe extern "system" fn bottom(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        BOTTOM.lock().unwrap().push(Seen { message, wparam: wparam.0, lparam: lparam.0 });
        if message == WM_SIZE && BLOCK_SIZE.load(Ordering::SeqCst) {
            IN_SIZE.store(true, Ordering::SeqCst);
            while BLOCK_SIZE.load(Ordering::SeqCst) {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            IN_SIZE.store(false, Ordering::SeqCst);
        }
        // SAFETY: the arguments of a live message
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }

    unsafe extern "system" fn top(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        TOP.lock().unwrap().push(Seen { message, wparam: wparam.0, lparam: lparam.0 });
        let previous = TOP_PREVIOUS.load(Ordering::SeqCst);
        // SAFETY: the address is the procedure the window had when `install_top` ran
        let previous = unsafe { std::mem::transmute::<usize, WNDPROC>(previous) };
        // SAFETY: the arguments of a live message
        unsafe { CallWindowProcW(previous, hwnd, message, wparam, lparam) }
    }
}
