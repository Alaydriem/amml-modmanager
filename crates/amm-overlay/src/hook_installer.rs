use std::ffi::c_void;
use std::sync::atomic::AtomicPtr;

use crate::hook_token::HookToken;
use crate::overlay_error::OverlayError;

/// Installs and removes the overlay's detours: AMML's `Api::hook` and `Api::unhook` in the game, a `retour` detour
/// in tests.
///
/// # Safety
///
/// The function an installer writes into `original` stays callable for the life of the process: after `unhook`
/// of its hook and after the installer is dropped. A thread can be inside a detour, or just entering it, when the
/// hook is removed, and it calls on through `original`. The AMML loader meets this, since it never frees its
/// trampolines.
pub unsafe trait HookInstaller {
    /// Detours `target` to `detour`, and writes the function the detour calls on into `original` before the detour
    /// can run.
    ///
    /// # Safety
    ///
    /// `target` is the start of a function that can be detoured, and `detour` an `extern "system"` function with
    /// the same signature.
    unsafe fn hook(
        &self,
        target: usize,
        detour: *const c_void,
        original: &'static AtomicPtr<c_void>,
    ) -> Result<HookToken, OverlayError>;

    /// Removes the hook of `token`. A call already inside the detour runs on, through `original`.
    fn unhook(&self, token: HookToken) -> Result<(), OverlayError>;
}
