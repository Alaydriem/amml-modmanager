use amm_abi::types::{AmmHandle, AmmStatus};

use amm_core::structs::input::GameViewport;

use crate::services::{Boundary, HostCell};

/// Table `game_picture`. A null `out_width` or `out_height` is `NullPointer`, and nothing is written.
///
/// # Safety
///
/// `out_width` and `out_height` are null or writable for the call.
pub unsafe extern "C" fn amm_game_picture(handle: AmmHandle, out_width: *mut u32, out_height: *mut u32) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let (out_width, out_height) = (Boundary::writable(out_width)?, Boundary::writable(out_height)?);
        let picture = HostCell::global().with(|runtime| runtime.game_picture(handle))?;
        // SAFETY: both out pointers are non-null and aligned, as checked, and writable for the call, as the
        // caller gives
        unsafe {
            out_width.write(picture.width);
            out_height.write(picture.height);
        }
        Ok(())
    }))
}

/// Table `set_game_viewport`. A null `rect` clears the viewport.
///
/// # Safety
///
/// `rect` is null or readable for the call.
pub unsafe extern "C" fn amm_set_game_viewport(handle: AmmHandle, rect: *const [f32; 4]) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        let viewport = if rect.is_null() {
            None
        } else if !rect.is_aligned() {
            return Err(AmmStatus::NullPointer);
        } else {
            // SAFETY: rect is non-null and aligned, as checked, and readable for the call, as the caller gives
            let rect = unsafe { rect.read() };
            Some(GameViewport::new(rect).ok_or(AmmStatus::InvalidArgument)?)
        };
        HostCell::global().with(|runtime| runtime.set_game_viewport(handle, viewport))
    }))
}
