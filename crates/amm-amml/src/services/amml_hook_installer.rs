#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::atomic::AtomicPtr;

use amm_overlay::{HookInstaller, HookToken, OverlayError};
use amml_sdk::{Api, HookHandle};

/// The overlay's swap chain hooks through AMML's `Api::hook` and `Api::unhook`.
pub struct AmmlHookInstaller {
    api: Api,
}

impl AmmlHookInstaller {
    pub fn new(api: Api) -> Self {
        Self { api }
    }
}

// SAFETY: the AMML hook engine never frees a detour or its trampoline and lives for the process, so the function
// it writes into `original` stays callable after `unhook` and after this installer is dropped
unsafe impl HookInstaller for AmmlHookInstaller {
    unsafe fn hook(
        &self,
        target: usize,
        detour: *const c_void,
        original: &'static AtomicPtr<c_void>,
    ) -> Result<HookToken, OverlayError> {
        // SAFETY: the caller gives a target that can be detoured and a detour of its signature; the overlay keeps
        // one `original` slot per hooked entry, so the slot is used for this hook only
        let hooked = unsafe { self.api.hook(target as u64, detour, 0, original) };
        hooked.map(|handle| HookToken(handle.0)).map_err(|error| OverlayError::Hook { reason: error.to_string() })
    }

    fn unhook(&self, token: HookToken) -> Result<(), OverlayError> {
        self.api.unhook(HookHandle(token.0)).map_err(|error| OverlayError::Hook { reason: error.to_string() })
    }
}
