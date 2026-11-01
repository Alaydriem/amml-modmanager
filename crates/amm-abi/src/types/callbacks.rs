use std::ffi::c_void;

use amm_core::traits::AbiSized;

use super::{AmmFrameInput, AmmFrameOutput};

/// The callbacks a mod gives at register. The manager copies this struct during `register`, so it need not
/// outlive that call. `user` is passed back unchanged. A null slot means the mod does not implement that callback.
/// `size` is `size_of` the struct the mod was built with; the manager reads only that many bytes.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmCallbacks {
    pub size: u32,
    pub user: *mut c_void,
    /// Render thread. Runs the mod egui pass and returns an `AmmStatus` value as an integer.
    /// `input` and everything it points to are valid only for this call. The mod fills `out`; the pointers in
    /// `out.mesh` must stay valid until the next `frame` call for this mod. It must not wait on a lock that the
    /// mod's game-thread code holds while calling the table.
    pub frame:
        Option<unsafe extern "C" fn(user: *mut c_void, input: *const AmmFrameInput, out: *mut AmmFrameOutput) -> i32>,
    /// Render thread. The host lost its textures. The mod must upload them again on its next frame.
    pub textures_lost: Option<unsafe extern "C" fn(user: *mut c_void)>,
}

impl AbiSized for AmmCallbacks {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self { size: size_of::<Self>() as u32, user: std::ptr::null_mut(), frame: None, textures_lost: None }
    }
}
