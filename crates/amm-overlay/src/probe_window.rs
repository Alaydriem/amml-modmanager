use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, UnregisterClassW, WINDOW_EX_STYLE, WNDCLASSEXW,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::{PCWSTR, w};

use crate::overlay_error::OverlayError;

/// A hidden window of our own for the probe swap chain, never the game's. It is destroyed, and its class
/// unregistered, on drop, on the thread that made it.
pub(crate) struct ProbeWindow {
    hwnd: HWND,
    instance: HINSTANCE,
}

impl ProbeWindow {
    const CLASS: PCWSTR = w!("AlaydriemModManagerOverlayProbe");
    const SIZE: i32 = 8;

    pub(crate) fn new() -> Result<Self, OverlayError> {
        // SAFETY: no name asks for the module of the process, which stays loaded
        let module = unsafe { GetModuleHandleW(None) }
            .map_err(|source| OverlayError::Probe { step: "module handle", source })?;
        let instance = HINSTANCE(module.0);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: instance,
            lpszClassName: Self::CLASS,
            ..Default::default()
        };
        // SAFETY: `class` lives for the call. A class left by an earlier probe makes this fail, and the window below
        // still finds that class
        unsafe { RegisterClassExW(&class) };
        // SAFETY: the class is registered and every string is static
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                Self::CLASS,
                w!(""),
                WS_OVERLAPPEDWINDOW,
                0,
                0,
                Self::SIZE,
                Self::SIZE,
                None,
                None,
                Some(instance),
                None,
            )
        }
        .map_err(|source| OverlayError::Probe { step: "probe window", source })?;
        Ok(Self { hwnd, instance })
    }

    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: the system calls a window procedure with the arguments of a live message
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

impl Drop for ProbeWindow {
    fn drop(&mut self) {
        // SAFETY: the window was made by this value on this thread and is destroyed once; the class is unregistered
        // after its last window is gone
        unsafe {
            let _ = DestroyWindow(self.hwnd);
            let _ = UnregisterClassW(Self::CLASS, Some(self.instance));
        }
    }
}
