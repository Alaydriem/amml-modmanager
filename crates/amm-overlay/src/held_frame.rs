use amm_ffi::types::AmmHostFrame;

/// The host frame of the current present between the mod pass and the manager pass of `SurfaceRenderer`: the frame,
/// the device epoch it was drawn for, and the first layer the manager pass draws.
pub(crate) struct HeldFrame {
    pub(crate) frame: AmmHostFrame,
    pub(crate) epoch: u64,
    pub(crate) manager_from: usize,
}

// SAFETY: the frame's pointers are read only by the renderer that holds this value, on the render thread, while the
// `OverlayHost` contract keeps them valid (until its next `frame` call, which only that renderer makes); the
// renderer moves between threads only behind its owner's lock
unsafe impl Send for HeldFrame {}
