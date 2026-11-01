use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostMessageW, SIZE_RESTORED, WM_SIZE};

/// A `WM_SIZE` posted to the game window, so the game takes a new render size the way it takes a window resize.
pub struct SizeMessage;

impl SizeMessage {
    /// Posts `WM_SIZE` / `SIZE_RESTORED` with `width` and `height` to `window`, and logs it with `reason`. Does
    /// nothing for a null window or a side of 0. A post that fails is logged.
    pub fn post(window: HWND, width: u32, height: u32, reason: &'static str) {
        if window.0.is_null() || width == 0 || height == 0 {
            return;
        }
        // SAFETY: a handle that is not a window makes the call fail, and the message carries no pointer
        let posted = unsafe {
            PostMessageW(Some(window), WM_SIZE, WPARAM(SIZE_RESTORED as usize), Self::size_param(width, height))
        };
        match posted {
            Ok(()) => {
                curia::info!("render size message posted", { width: width, height: height, reason: reason });
            }
            Err(error) => {
                curia::warn!("render size message not posted", { error: error.to_string() });
            }
        }
    }

    /// The `lParam` of a `WM_SIZE` of `width` x `height`: the width in the low word, the height in the high word.
    pub(crate) fn size_param(width: u32, height: u32) -> LPARAM {
        LPARAM((((height & 0xFFFF) << 16) | (width & 0xFFFF)) as isize)
    }
}
