use amm_abi::types::{AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus};
use amm_core::structs::render::RenderSize;

use crate::services::{Boundary, HostCell};

/// Table `set_render_size`. A null `size` ends the forced size.
///
/// # Safety
///
/// `size` is null or readable for the call.
pub unsafe extern "C" fn amm_set_render_size(handle: AmmHandle, size: *const AmmRenderSize) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let size = if size.is_null() {
            None
        } else if !size.is_aligned() {
            return Err(AmmStatus::NullPointer);
        } else {
            // SAFETY: size is non-null and aligned, as checked, and readable for the call, as the caller gives
            Some(RenderSize::from(unsafe { size.read() }))
        };
        HostCell::global().with(|runtime| runtime.set_render_size(handle, size))
    }))
}

/// Table `render_size`. A null `out_state` is `NullPointer` and a `size` below the 2.0 struct is `InvalidArgument`,
/// and nothing is written. The state is written up to the `size` the caller declares, which then holds the bytes
/// written.
///
/// # Safety
///
/// `out_state` is null or points at a state that holds the bytes its `size` declares, writable for the call.
pub unsafe extern "C" fn amm_render_size(handle: AmmHandle, out_state: *mut AmmRenderSizeState) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let out_state = Boundary::writable(out_state)?;
        let state = HostCell::global().with(|runtime| runtime.render_size_state(handle))?;
        // SAFETY: the caller gives a state of its declared size, writable for the call
        unsafe { Boundary::sized_output(out_state, &AmmRenderSizeState::from(state)) }
    }))
}
