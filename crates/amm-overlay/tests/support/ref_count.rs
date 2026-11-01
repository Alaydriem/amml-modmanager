use windows::core::{IUnknown, Interface};

/// The reference count of a COM object, read by a balanced AddRef and Release.
pub struct RefCount;

impl RefCount {
    pub fn of(object: &impl Interface) -> u32 {
        let unknown: IUnknown = object.cast().unwrap();
        let raw = unknown.as_raw();
        let vtable = unknown.vtable();
        // SAFETY: the object is live; the AddRef is undone by the Release, and the cast's own reference is not counted
        unsafe {
            (vtable.AddRef)(raw);
            (vtable.Release)(raw) - 1
        }
    }
}
