use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use super::ThreadCount;

/// The system allocator, counting the allocations of the calling thread only. The host starts its own threads (the
/// I/O thread, the log writers); their work is not part of the frame path, so a measurement made on the frame thread
/// must not see it. The counters are per thread and need no allocation, so counting cannot recurse.
pub struct ThreadAlloc;

thread_local! {
    static COUNT: Cell<ThreadCount> = const { Cell::new(ThreadCount { allocations: 0, reallocations: 0, bytes_allocated: 0 }) };
}

impl ThreadAlloc {
    /// The counts of the calling thread so far.
    pub fn current() -> ThreadCount {
        COUNT.try_with(Cell::get).unwrap_or_default()
    }

    /// The counts of the calling thread since `start`.
    pub fn since(start: ThreadCount) -> ThreadCount {
        let now = Self::current();
        ThreadCount {
            allocations: now.allocations - start.allocations,
            reallocations: now.reallocations - start.reallocations,
            bytes_allocated: now.bytes_allocated - start.bytes_allocated,
        }
    }

    // A thread that is being torn down has no counter left; its allocations are not counted.
    fn record(allocations: usize, reallocations: usize, bytes: usize) {
        let _ = COUNT.try_with(|count| {
            let mut value = count.get();
            value.allocations += allocations;
            value.reallocations += reallocations;
            value.bytes_allocated += bytes;
            count.set(value);
        });
    }
}

// SAFETY: every call forwards to the system allocator with the caller's arguments unchanged, so the GlobalAlloc
// contract the caller upholds for `System` is the one upheld here; the counting touches only a thread-local Cell
unsafe impl GlobalAlloc for ThreadAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        Self::record(1, 0, layout.size());
        // SAFETY: the caller upholds the GlobalAlloc::alloc contract for this layout
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        Self::record(1, 0, layout.size());
        // SAFETY: the caller upholds the GlobalAlloc::alloc_zeroed contract for this layout
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: the caller gives a pointer this allocator returned for this layout
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        Self::record(0, 1, new_size.saturating_sub(layout.size()));
        // SAFETY: the caller gives a pointer this allocator returned for this layout and a valid new size
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}
