#![allow(unsafe_code)]

use amml_msvc::MsvcHeap;
use amml_sdk::ModRuntime;

/// The game heap through the AMML loader's `Api::heap_new` and `Api::heap_delete`, as an `MsvcHeap`, so a string
/// the game allocated is freed the way the game's STL frees it. The loader checked its heap entries at load.
pub struct LoaderHeap;

impl LoaderHeap {
    /// The heap; `None` before `amml_mod_load` installed the loader table.
    pub fn msvc() -> Option<MsvcHeap> {
        ModRuntime::get()?;
        Some(MsvcHeap::new(Self::allocate, Self::free))
    }

    unsafe extern "C" fn allocate(size: usize) -> *mut u8 {
        ModRuntime::get().map_or(std::ptr::null_mut(), |runtime| runtime.api().heap_new(size))
    }

    unsafe extern "C" fn free(ptr: *mut u8, size: usize) {
        if let Some(runtime) = ModRuntime::get() {
            // SAFETY: `MsvcHeap` passes a block of the game heap with the size it was allocated with; the loader's
            // `heap_delete` hands it to the game's operator delete, which ignores the size
            unsafe { runtime.api().heap_delete(ptr, size) };
        }
    }
}
