use std::ffi::c_void;
use std::marker::PhantomData;

use amm_abi::types::{AmmCallbacks, AmmFrameInput, AmmFrameOutput, AmmStatus};
use amm_core::services::PanicGuard;
use amm_core::traits::AbiSized;

use super::EguiFrame;
use crate::link::{FrameHandler, SizedAbi};

/// The `extern "C"` callbacks the manager calls for one handler type. `user` is a boxed `EguiFrame<H>` made by
/// `into_user`. No panic crosses these functions.
pub struct FrameTrampoline<H: FrameHandler> {
    handler: PhantomData<H>,
}

impl<H: FrameHandler> FrameTrampoline<H> {
    /// Boxes `frame` and returns the pointer the callbacks take as `user`. `free` gives it back.
    pub fn into_user(frame: EguiFrame<H>) -> *mut c_void {
        Box::into_raw(Box::new(frame)).cast::<c_void>()
    }

    /// The callbacks struct for a `user` pointer from `into_user`.
    pub fn callbacks(user: *mut c_void) -> AmmCallbacks {
        AmmCallbacks {
            user,
            frame: Some(Self::frame),
            textures_lost: Some(Self::textures_lost),
            ..AmmCallbacks::blank()
        }
    }

    /// Render thread. Runs one pass and returns an `AmmStatus` value.
    ///
    /// # Safety
    ///
    /// `user` comes from `into_user` and is not yet freed, no other call uses it at the same time, and `input`
    /// and `out` are null or valid for the call, each holding the bytes its `size` declares, as the manager's frame
    /// contract says. An `input` or `out` whose `size` is below the 2.0 struct is `InvalidArgument`, and no pass
    /// runs.
    pub unsafe extern "C" fn frame(user: *mut c_void, input: *const AmmFrameInput, out: *mut AmmFrameOutput) -> i32 {
        if user.is_null() || input.is_null() || out.is_null() {
            return AmmStatus::NullPointer as i32;
        }
        // SAFETY: the caller gives an input of its declared size for the call
        let input = match unsafe { SizedAbi::read(input) } {
            Ok(input) => input,
            Err(status) => return status as i32,
        };
        // SAFETY: the caller gives an out struct of its declared size for the call; this only reads it, so a bad size
        // is refused before the pass runs
        if let Err(status) = unsafe { SizedAbi::read(out.cast_const()) } {
            return status as i32;
        }
        let frame = user.cast::<EguiFrame<H>>();
        let ran = PanicGuard::run(
            || {
                let mut filled = AmmFrameOutput::blank();
                // SAFETY: the caller guarantees user is a live EguiFrame<H> used by this call alone, and that the
                // input's events are valid for the call
                let status = unsafe { (*frame).run(&input, &mut filled) };
                // SAFETY: the caller gives an out struct of its declared size for the call, checked above
                unsafe { SizedAbi::write(out, &filled) }.map(|()| status).unwrap_or_else(|status| status)
            },
            |_| {
                // SAFETY: the panic was caught, so the frame state is still owned here and nothing else holds it
                if unsafe { (*frame).recover() } {
                    curia::error!("frame handler panicked, its frames are dropped while it keeps failing");
                }
            },
        );
        ran.unwrap_or(AmmStatus::Panic) as i32
    }

    /// Render thread. The host lost its textures.
    ///
    /// # Safety
    ///
    /// `user` comes from `into_user`, is not yet freed, and no other call uses it at the same time.
    pub unsafe extern "C" fn textures_lost(user: *mut c_void) {
        if user.is_null() {
            return;
        }
        let frame = user.cast::<EguiFrame<H>>();
        PanicGuard::run(
            || {
                // SAFETY: the caller guarantees user is a live EguiFrame<H> used by this call alone
                unsafe { (*frame).textures_lost() }
            },
            |_| curia::error!("frame handler panicked while its textures were reset"),
        );
    }

    /// Drops the frame state behind `user`.
    ///
    /// # Safety
    ///
    /// `user` comes from `into_user`, is freed once, and the manager no longer calls the callbacks with it.
    pub unsafe fn free(user: *mut c_void) {
        if user.is_null() {
            return;
        }
        // SAFETY: the caller guarantees user is the box from into_user, freed this once with no call in flight
        drop(unsafe { Box::from_raw(user.cast::<EguiFrame<H>>()) });
    }
}
