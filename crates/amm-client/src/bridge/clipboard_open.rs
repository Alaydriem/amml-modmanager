use std::marker::PhantomData;

use windows_sys::Win32::System::DataExchange::{CloseClipboard, OpenClipboard};

use super::ClipboardFault;

/// The Windows clipboard, open on this thread until the guard drops. The guard cannot leave the thread that
/// opened it, because Windows ties an open clipboard to that thread.
pub(crate) struct ClipboardOpen {
    thread_bound: PhantomData<*const ()>,
}

impl ClipboardOpen {
    /// Opens the clipboard. `Busy` when another program holds it open.
    pub(crate) fn new() -> Result<Self, ClipboardFault> {
        // SAFETY: a null owner is allowed and ties the clipboard to this thread; the Drop below closes it
        if unsafe { OpenClipboard(std::ptr::null_mut()) } == 0 {
            return Err(ClipboardFault::Busy);
        }
        Ok(Self { thread_bound: PhantomData })
    }
}

impl Drop for ClipboardOpen {
    fn drop(&mut self) {
        // SAFETY: this guard opened the clipboard on this thread and has not closed it
        unsafe { CloseClipboard() };
    }
}
