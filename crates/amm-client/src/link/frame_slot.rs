use std::ffi::c_void;

use amm_abi::types::AmmCallbacks;

/// The boxed frame state a link's layer callbacks take as `user`, and the function that frees it. The manager gets
/// the callbacks at connect; the state is freed once the manager can no longer call them (an `Ok` unregister), or
/// when it never got them.
pub(crate) struct FrameSlot {
    callbacks: AmmCallbacks,
    free: unsafe fn(*mut c_void),
    given: bool,
}

// SAFETY: the state behind `callbacks.user` holds an `H: Send` and is reached only by the manager's render callbacks
// and by `free`, which runs once no callback can run
unsafe impl Send for FrameSlot {}

impl FrameSlot {
    // Only a link with an egui frame handler has frame state
    #[cfg(feature = "egui")]
    pub(crate) fn new(callbacks: AmmCallbacks, free: unsafe fn(*mut c_void)) -> Self {
        Self { callbacks, free, given: false }
    }

    pub(crate) fn callbacks(&self) -> &AmmCallbacks {
        &self.callbacks
    }

    /// The manager now holds a copy of the callbacks.
    pub(crate) fn mark_given(&mut self) {
        self.given = true;
    }

    /// Whether the manager may hold a copy of the callbacks.
    pub(crate) fn given(&self) -> bool {
        self.given
    }

    /// Frees the frame state.
    ///
    /// # Safety
    ///
    /// No callback with this state runs now or later: the manager never got the callbacks, or it answered `Ok` to
    /// unregister.
    pub(crate) unsafe fn free(self) {
        // SAFETY: the caller guarantees no callback uses the state, and `self` is consumed, so it is freed once
        unsafe { (self.free)(self.callbacks.user) };
    }
}
