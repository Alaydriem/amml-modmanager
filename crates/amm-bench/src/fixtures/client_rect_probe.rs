#![allow(unsafe_code)]

use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, GetClientRect, WS_POPUP};

/// A hidden borderless window, for timing the `GetClientRect` the overlay calls on every Present to give the host the
/// window's client size.
pub struct ClientRectProbe {
    window: HWND,
}

impl ClientRectProbe {
    pub const WIDTH: u32 = 3840;
    pub const HEIGHT: u32 = 2160;
    const CLASS: &str = "STATIC";

    pub fn new() -> Self {
        let class: Vec<u16> = Self::CLASS.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the class name is a NUL-terminated wide string that lives for the call; the other pointers are
        // null, which CreateWindowExW accepts
        let window = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                std::ptr::null(),
                WS_POPUP,
                0,
                0,
                Self::WIDTH as i32,
                Self::HEIGHT as i32,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        assert!(!window.is_null(), "CreateWindowExW failed");
        Self { window }
    }

    /// The client size, as the overlay reads it; 0 x 0 when the call fails.
    pub fn read(&self) -> (u32, u32) {
        let mut client = RECT { left: 0, top: 0, right: 0, bottom: 0 };
        // SAFETY: the window lives until drop, and client is writable for the call
        if unsafe { GetClientRect(self.window, &mut client) } == 0 {
            return (0, 0);
        }
        (client.right.max(0) as u32, client.bottom.max(0) as u32)
    }
}

impl Default for ClientRectProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ClientRectProbe {
    fn drop(&mut self) {
        // SAFETY: the window was made by this probe and is destroyed once
        unsafe { DestroyWindow(self.window) };
    }
}
