use super::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmHandle};

/// The registry table `alaydriem.modmanager.keybinds`: the key binds of the mod's descriptor as the person set them.
/// Game thread. Presses come as `Keybind` events from `poll_events`. New functions are only added at the end, with a
/// higher minor. A null slot means the function is absent. After the manager is disabled every function answers 0.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmKeybindsTable {
    pub size: u32,
    pub version: u32,
    /// The person's chord of descriptor key bind `index` as text (for example `Ctrl+Shift+F10`), copied into
    /// `out_buf` (room for `out_cap` bytes); returns its length. A return value greater than `out_cap` means the
    /// buffer was too small and nothing was written. `out_buf` may be null when `out_cap` is 0. An index past the
    /// descriptor's list or a handle that names no live registration gives 0.
    pub chord_text:
        Option<unsafe extern "C" fn(handle: AmmHandle, index: u32, out_buf: *mut u8, out_cap: usize) -> u32>,
    /// 1 while descriptor key bind `index` of the mod is held down (between its `Pressed` and `Released` events),
    /// else 0. An index past the descriptor's list or a handle that names no live registration gives 0.
    pub bind_down: Option<unsafe extern "C" fn(handle: AmmHandle, index: u32) -> u8>,
}

// SAFETY: `AmmKeybindsTable` is `#[repr(C)]` and starts with `size: u32, version: u32`, and `size` is the byte size
// of the provider's table.
#[allow(unsafe_code)]
unsafe impl amml_api::Provided for AmmKeybindsTable {
    const NAME: &'static str = "alaydriem.modmanager.keybinds";
    const MAJOR: u16 = AMM_ABI_MAJOR as u16;
    const MINOR: u16 = AMM_ABI_MINOR as u16;
}
