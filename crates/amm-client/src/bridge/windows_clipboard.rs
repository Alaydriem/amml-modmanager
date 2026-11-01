use windows_sys::Win32::Foundation::GlobalFree;
use windows_sys::Win32::System::DataExchange::{
    EmptyClipboard, GetClipboardData, IsClipboardFormatAvailable, SetClipboardData,
};
use windows_sys::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};

use super::{ClipboardFault, ClipboardOpen, ClipboardWorker};

/// Text on the Windows clipboard, as UTF-16 in the `CF_UNICODETEXT` format. Each call opens the clipboard once
/// and closes it before it returns.
pub(crate) struct WindowsClipboard;

impl WindowsClipboard {
    const CF_UNICODETEXT: u32 = 13;
    const HIGH_SURROGATES: std::ops::RangeInclusive<u16> = 0xD800..=0xDBFF;

    /// Puts `text` on the clipboard in place of what was there.
    pub(crate) fn write(text: &str) -> Result<(), ClipboardFault> {
        let _open = ClipboardOpen::new()?;
        // SAFETY: the clipboard is open on this thread while `_open` lives
        if unsafe { EmptyClipboard() } == 0 {
            return Err(ClipboardFault::Failed);
        }
        let units = text.encode_utf16().count() + 1;
        // SAFETY: GlobalAlloc takes any flags and size and returns null on failure, which is checked below
        let handle = unsafe { GlobalAlloc(GMEM_MOVEABLE, units * size_of::<u16>()) };
        if handle.is_null() {
            return Err(ClipboardFault::Failed);
        }
        // SAFETY: handle is the live movable block GlobalAlloc just returned
        let block = unsafe { GlobalLock(handle) }.cast::<u16>();
        if block.is_null() {
            // SAFETY: handle is unlocked, not given to the system, and freed once here
            unsafe { GlobalFree(handle) };
            return Err(ClipboardFault::Failed);
        }
        // SAFETY: the locked block holds `units` u16 values (len + 1) * 2 bytes, GlobalLock memory is aligned for
        // u16, and nothing else refers to it while it is locked
        let target = unsafe { std::slice::from_raw_parts_mut(block, units) };
        for (slot, unit) in target.iter_mut().zip(text.encode_utf16().chain(std::iter::once(0))) {
            *slot = unit;
        }
        // SAFETY: handle was locked once above and `target` is not used after this
        unsafe { GlobalUnlock(handle) };
        // SAFETY: the clipboard is open on this thread and was emptied, and handle is an unlocked movable block
        // holding NUL-terminated UTF-16
        if unsafe { SetClipboardData(Self::CF_UNICODETEXT, handle) }.is_null() {
            // SAFETY: the system did not take handle, so it is still ours and freed once here
            unsafe { GlobalFree(handle) };
            return Err(ClipboardFault::Failed);
        }
        Ok(())
    }

    /// The text on the clipboard, cut to at most `ClipboardWorker::MAX_PASTE_BYTES` UTF-16 units. Empty when the
    /// clipboard holds no text.
    pub(crate) fn read() -> Result<String, ClipboardFault> {
        let _open = ClipboardOpen::new()?;
        // SAFETY: the clipboard is open on this thread while `_open` lives
        if unsafe { IsClipboardFormatAvailable(Self::CF_UNICODETEXT) } == 0 {
            return Ok(String::new());
        }
        // SAFETY: the clipboard is open on this thread and holds CF_UNICODETEXT
        let handle = unsafe { GetClipboardData(Self::CF_UNICODETEXT) };
        if handle.is_null() {
            return Err(ClipboardFault::Failed);
        }
        // SAFETY: handle is the clipboard's block, which stays valid while the clipboard is open
        let block = unsafe { GlobalLock(handle) }.cast::<u16>().cast_const();
        if block.is_null() {
            return Err(ClipboardFault::Failed);
        }
        // SAFETY: handle is a live block, locked above
        let units = unsafe { GlobalSize(handle) } / size_of::<u16>();
        // SAFETY: the locked block holds GlobalSize bytes, so `units` u16 values, GlobalLock memory is aligned for
        // u16, and the clipboard owner cannot change it while this thread holds the clipboard open
        let text = Self::decode(unsafe { std::slice::from_raw_parts(block, units) });
        // SAFETY: handle was locked once above and the slice is not used after this
        unsafe { GlobalUnlock(handle) };
        Ok(text)
    }

    // Stops at the first NUL and at MAX_PASTE_BYTES units, so a large clipboard is never decoded whole; a high
    // surrogate left at the cut is dropped
    fn decode(units: &[u16]) -> String {
        let end = units.iter().position(|&unit| unit == 0).unwrap_or(units.len());
        let mut text = &units[..end.min(ClipboardWorker::MAX_PASTE_BYTES)];
        if let Some((last, rest)) = text.split_last()
            && Self::HIGH_SURROGATES.contains(last)
        {
            text = rest;
        }
        String::from_utf16_lossy(text)
    }
}
