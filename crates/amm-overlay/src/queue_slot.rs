use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Graphics::Direct3D12::ID3D12CommandQueue;
use windows::core::Interface;

/// The game's first D3D12 DIRECT queue, seen in `ExecuteCommandLists`, with one reference held. The D3D11On12
/// device draws through it. Any thread may offer a queue; only the render thread and shutdown, after the drain,
/// take it away.
pub(crate) struct QueueSlot {
    queue: AtomicPtr<c_void>,
}

impl QueueSlot {
    pub(crate) const fn new() -> Self {
        Self { queue: AtomicPtr::new(std::ptr::null_mut()) }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.queue.load(Ordering::SeqCst).is_null()
    }

    /// Keeps `queue` when the slot is empty.
    pub(crate) fn offer(&self, queue: &ID3D12CommandQueue) {
        if !self.is_empty() {
            return;
        }
        let raw = queue.clone().into_raw();
        if self.queue.compare_exchange(std::ptr::null_mut(), raw, Ordering::SeqCst, Ordering::SeqCst).is_err() {
            // SAFETY: `raw` holds the reference taken above, which was not stored
            drop(unsafe { ID3D12CommandQueue::from_raw(raw) });
        }
    }

    /// A new reference to the kept queue. Render thread only.
    pub(crate) fn get(&self) -> Option<ID3D12CommandQueue> {
        let raw = self.queue.load(Ordering::SeqCst);
        // SAFETY: a stored pointer holds the slot's own reference, and only the render thread (the caller) or
        // shutdown after the drain releases it
        unsafe { ID3D12CommandQueue::from_raw_borrowed(&raw) }.cloned()
    }

    /// Drops `queue` when it is still the one kept, so the next submit can be captured.
    pub(crate) fn forget(&self, queue: &ID3D12CommandQueue) {
        let raw = queue.as_raw();
        if self.queue.compare_exchange(raw, std::ptr::null_mut(), Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            // SAFETY: the slot held one reference to `raw`, now taken out of it
            drop(unsafe { ID3D12CommandQueue::from_raw(raw) });
        }
    }

    /// Drops the kept queue, if any.
    pub(crate) fn clear(&self) {
        let raw = self.queue.swap(std::ptr::null_mut(), Ordering::SeqCst);
        if !raw.is_null() {
            // SAFETY: the slot held one reference to `raw`, now taken out of it
            drop(unsafe { ID3D12CommandQueue::from_raw(raw) });
        }
    }
}
