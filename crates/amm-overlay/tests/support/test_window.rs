use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassExW, WINDOW_EX_STYLE, WNDCLASSEXW, WS_POPUP,
};
use windows::core::{PCWSTR, w};

/// A hidden window for a test swap chain, destroyed on drop.
pub struct TestWindow {
    pub hwnd: HWND,
}

impl TestWindow {
    const CLASS: PCWSTR = w!("AmmOverlayTestWindow");

    pub fn new(size: u32) -> Self {
        // SAFETY: the module of the process stays loaded
        let instance = HINSTANCE(unsafe { GetModuleHandleW(None) }.unwrap().0);
        let class = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(Self::window_proc),
            hInstance: instance,
            lpszClassName: Self::CLASS,
            ..Default::default()
        };
        // SAFETY: class lives for the call; a second registration fails and the class stays usable
        unsafe { RegisterClassExW(&class) };
        // SAFETY: the class is registered and the strings are static
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                Self::CLASS,
                w!(""),
                WS_POPUP,
                0,
                0,
                size as i32,
                size as i32,
                None,
                None,
                Some(instance),
                None,
            )
        }
        .unwrap();
        Self { hwnd }
    }

    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        // SAFETY: the arguments of a live message
        unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
    }
}

impl Drop for TestWindow {
    fn drop(&mut self) {
        // SAFETY: made by this value on this thread
        let _ = unsafe { DestroyWindow(self.hwnd) };
    }
}
