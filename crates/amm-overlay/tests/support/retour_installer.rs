use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicPtr, Ordering};

use amm_overlay::{HookInstaller, HookToken, OverlayError};
use retour::RawDetour;

/// A `HookInstaller` over `retour`. Every detour is leaked: an unhooked one is disabled but its trampoline stays for
/// the life of the process, as the `HookInstaller` contract asks.
pub struct RetourInstaller {
    detours: Mutex<Vec<&'static RawDetour>>,
}

impl RetourInstaller {
    pub fn new() -> Self {
        Self { detours: Mutex::new(Vec::new()) }
    }

    fn error(error: retour::Error) -> OverlayError {
        OverlayError::Hook { reason: error.to_string() }
    }
}

// SAFETY: the detours are leaked, so every trampoline written to `original` lives for the process
unsafe impl HookInstaller for RetourInstaller {
    unsafe fn hook(
        &self,
        target: usize,
        detour: *const c_void,
        original: &'static AtomicPtr<c_void>,
    ) -> Result<HookToken, OverlayError> {
        // SAFETY: the caller vouches for the target and the detour
        let hook = unsafe { RawDetour::new(target as *const (), detour.cast()) }.map_err(Self::error)?;
        let hook: &'static RawDetour = Box::leak(Box::new(hook));
        original.store(std::ptr::from_ref(hook.trampoline()).cast_mut().cast(), Ordering::SeqCst);
        // SAFETY: the target was checked by RawDetour::new and `original` is written
        unsafe { hook.enable() }.map_err(Self::error)?;
        let mut detours = self.detours.lock().unwrap_or_else(|poison| poison.into_inner());
        detours.push(hook);
        Ok(HookToken((detours.len() - 1) as u64))
    }

    fn unhook(&self, token: HookToken) -> Result<(), OverlayError> {
        let detours = self.detours.lock().unwrap_or_else(|poison| poison.into_inner());
        let Some(hook) = detours.get(token.0 as usize) else {
            return Err(OverlayError::Hook { reason: "unknown token".to_string() });
        };
        // SAFETY: the detour was enabled by `hook`
        unsafe { hook.disable() }.map_err(Self::error)
    }
}
