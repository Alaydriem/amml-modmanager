use amm_abi::types::AmmStatus;

use crate::services::{Boundary, HostCell};
use crate::types::{AmmHostFrame, AmmHostFrameInput};

/// Render thread, inside Present. Fills `out` with the layers to draw, valid until the next call.
///
/// # Safety
///
/// `input` is null or a live `AmmHostFrameInput`, and `out` is null or writable, for the call.
pub unsafe extern "C" fn amm_host_frame(input: *const AmmHostFrameInput, out: *mut AmmHostFrame) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives a null or live input for the call
        let input = unsafe { Boundary::input(input) }?;
        let out = Boundary::writable(out)?;
        let frame = HostCell::global().frame(input)?;
        // SAFETY: out is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out.write(frame) };
        Ok(())
    }))
}

/// Render thread. The device dropped every texture: each mod and the panel send theirs again on the next frame.
pub extern "C" fn amm_host_textures_lost() {
    let _ = Boundary::guard(|| {
        HostCell::global().with(|runtime| {
            runtime.textures_lost();
            Ok(())
        })
    });
}
