use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Dxgi::{DXGI_ERROR_INVALID_CALL, DXGI_PRESENT_PARAMETERS, DXGI_PRESENT_TEST};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;
use windows::core::HRESULT;

use crate::hook_installer::HookInstaller;
use crate::hook_token::HookToken;
use crate::overlay_error::OverlayError;
use crate::overlay_shared::OverlayShared;
use crate::pace_clock::PaceClock;
use crate::present_depth::PresentDepth;
use crate::probe_targets::ProbeTargets;

type PresentFn = unsafe extern "system" fn(*mut c_void, u32, u32) -> HRESULT;
type Present1Fn = unsafe extern "system" fn(*mut c_void, u32, u32, *const DXGI_PRESENT_PARAMETERS) -> HRESULT;
type ResizeBuffersFn = unsafe extern "system" fn(*mut c_void, u32, u32, u32, DXGI_FORMAT, u32) -> HRESULT;
type ResizeBuffers1Fn =
    unsafe extern "system" fn(*mut c_void, u32, u32, u32, DXGI_FORMAT, u32, *const u32, *const *mut c_void) -> HRESULT;
type ExecuteCommandListsFn = unsafe extern "system" fn(*mut c_void, u32, *const *mut c_void);

static ORIGINAL_PRESENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_PRESENT1: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_RESIZE_BUFFERS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_RESIZE_BUFFERS1: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static ORIGINAL_EXECUTE_COMMAND_LISTS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// The five detours and their install. Each detour counts itself in flight, runs its overlay part inside
/// `OverlayShared::guard`, and then calls the original exactly once, also after a panic or while the overlay is
/// off. The resize detours run their part also while the overlay is off, so a target left bound is still
/// released before the original. Every hook calls on to the function installed before it.
pub(crate) struct PresentHooks;

impl PresentHooks {
    /// Hooks the entries of `targets`. The queue entry goes first, so the D3D12 queue can be seen before any
    /// Present, then both resizes, then both presents, so no chain is drawn on before its resize is hooked. A
    /// required entry that fails removes the ones already in place; the queue entry is optional.
    pub(crate) fn install<I: HookInstaller>(
        installer: &I,
        targets: &ProbeTargets,
    ) -> Result<Vec<HookToken>, OverlayError> {
        let mut tokens = Vec::with_capacity(5);
        if let Some(target) = targets.execute_command_lists {
            // SAFETY: the target is ExecuteCommandLists from the probe queue's vtable, and the detour has its
            // signature
            match unsafe {
                installer.hook(
                    target,
                    Self::execute_command_lists as ExecuteCommandListsFn as *const c_void,
                    &ORIGINAL_EXECUTE_COMMAND_LISTS,
                )
            } {
                Ok(token) => tokens.push(token),
                Err(error) => {
                    curia::warn!("overlay has no queue hook, it can draw only on a D3D11 game", {
                        error: error.to_string()
                    });
                }
            }
        }
        let required: [(&'static str, usize, *const c_void, &'static AtomicPtr<c_void>); 4] = [
            (
                "ResizeBuffers",
                targets.resize_buffers,
                Self::resize_buffers as ResizeBuffersFn as *const c_void,
                &ORIGINAL_RESIZE_BUFFERS,
            ),
            (
                "ResizeBuffers1",
                targets.resize_buffers1,
                Self::resize_buffers1 as ResizeBuffers1Fn as *const c_void,
                &ORIGINAL_RESIZE_BUFFERS1,
            ),
            ("Present", targets.present, Self::present as PresentFn as *const c_void, &ORIGINAL_PRESENT),
            ("Present1", targets.present1, Self::present1 as Present1Fn as *const c_void, &ORIGINAL_PRESENT1),
        ];
        for (entry, target, detour, original) in required {
            // SAFETY: the target is the swap chain entry of the probe chain's vtable that `entry` names, and the
            // detour has its signature
            match unsafe { installer.hook(target, detour, original) } {
                Ok(token) => tokens.push(token),
                Err(error) => {
                    curia::warn!("overlay hook failed", { entry: entry, error: error.to_string() });
                    Self::uninstall(installer, &mut tokens);
                    return Err(error);
                }
            }
        }
        Ok(tokens)
    }

    /// Removes every hook of `tokens`. A failure is logged; that detour stays, and finds the overlay shut.
    pub(crate) fn uninstall<I: HookInstaller>(installer: &I, tokens: &mut Vec<HookToken>) {
        for token in tokens.drain(..).rev() {
            if let Err(error) = installer.unhook(token) {
                curia::warn!("overlay unhook failed", { error: error.to_string() });
            }
        }
    }

    unsafe extern "system" fn present(chain: *mut c_void, sync: u32, flags: u32) -> HRESULT {
        let shared = OverlayShared::global();
        let _call = shared.in_flight.enter();
        let depth = PresentDepth::enter();
        shared.guard("Present", (), || shared.present(chain, flags, depth.outermost()));
        let Some(original) = Self::original::<PresentFn>(&ORIGINAL_PRESENT) else {
            return DXGI_ERROR_INVALID_CALL;
        };
        let started = PaceClock::ticks();
        // SAFETY: the original takes the arguments DXGI gave this detour
        let result = unsafe { original(chain, sync, flags) };
        Self::observe_pace(shared, &depth, sync, flags, result, started);
        result
    }

    unsafe extern "system" fn present1(
        chain: *mut c_void,
        sync: u32,
        flags: u32,
        params: *const DXGI_PRESENT_PARAMETERS,
    ) -> HRESULT {
        let shared = OverlayShared::global();
        let _call = shared.in_flight.enter();
        let depth = PresentDepth::enter();
        shared.guard("Present1", (), || shared.present(chain, flags, depth.outermost()));
        let Some(original) = Self::original::<Present1Fn>(&ORIGINAL_PRESENT1) else {
            return DXGI_ERROR_INVALID_CALL;
        };
        let started = PaceClock::ticks();
        // SAFETY: as for present
        let result = unsafe { original(chain, sync, flags, params) };
        Self::observe_pace(shared, &depth, sync, flags, result, started);
        result
    }

    unsafe extern "system" fn resize_buffers(
        chain: *mut c_void,
        count: u32,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        flags: u32,
    ) -> HRESULT {
        let shared = OverlayShared::global();
        let _call = shared.in_flight.enter();
        let given = shared.contain("ResizeBuffers", None, || shared.before_resize(chain, width, height));
        let bound = given.is_some();
        let (width, height) = given.unwrap_or((width, height));
        let Some(original) = Self::original::<ResizeBuffersFn>(&ORIGINAL_RESIZE_BUFFERS) else {
            return DXGI_ERROR_INVALID_CALL;
        };
        // SAFETY: as for present
        let result = unsafe { original(chain, count, width, height, format, flags) };
        if bound && result.is_ok() {
            shared.contain("ResizeBuffers", (), || shared.after_resize(chain));
        }
        result
    }

    #[allow(clippy::too_many_arguments)]
    unsafe extern "system" fn resize_buffers1(
        chain: *mut c_void,
        count: u32,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
        flags: u32,
        node_masks: *const u32,
        queues: *const *mut c_void,
    ) -> HRESULT {
        let shared = OverlayShared::global();
        let _call = shared.in_flight.enter();
        let given = shared.contain("ResizeBuffers1", None, || shared.before_resize(chain, width, height));
        let bound = given.is_some();
        let (width, height) = given.unwrap_or((width, height));
        let Some(original) = Self::original::<ResizeBuffers1Fn>(&ORIGINAL_RESIZE_BUFFERS1) else {
            return DXGI_ERROR_INVALID_CALL;
        };
        // SAFETY: as for present
        let result = unsafe { original(chain, count, width, height, format, flags, node_masks, queues) };
        if bound && result.is_ok() {
            shared.contain("ResizeBuffers1", (), || shared.after_resize(chain));
        }
        result
    }

    unsafe extern "system" fn execute_command_lists(queue: *mut c_void, count: u32, lists: *const *mut c_void) {
        let shared = OverlayShared::global();
        let _call = shared.in_flight.enter();
        shared.guard("ExecuteCommandLists", (), || shared.offer_queue(queue));
        if let Some(original) = Self::original::<ExecuteCommandListsFn>(&ORIGINAL_EXECUTE_COMMAND_LISTS) {
            // SAFETY: as for present
            unsafe { original(queue, count, lists) };
        }
    }

    // the finished Present to the pace log, with the time spent inside the original, and whether the game window is
    // the foreground window (a read-only query)
    fn observe_pace(
        shared: &OverlayShared,
        depth: &PresentDepth,
        sync: u32,
        flags: u32,
        result: HRESULT,
        started: i64,
    ) {
        let inside_us = PaceClock::micros_since(started);
        shared.guard("Present", (), || {
            let counts = depth.outermost() && flags & DXGI_PRESENT_TEST.0 == 0;
            let window = shared.game_window();
            // SAFETY: GetForegroundWindow takes no arguments, changes nothing and may return a null handle
            let foreground = (window != 0).then(|| unsafe { GetForegroundWindow() }.0 as usize == window);
            shared.pace.observe(sync, flags, result, inside_us, counts, foreground);
        });
    }

    // `F` is the function pointer type of the entry the slot belongs to
    fn original<F: Copy>(slot: &AtomicPtr<c_void>) -> Option<F> {
        // Acquire pairs with the installer's store of the slot, made before the detour was enabled
        let original = slot.load(Ordering::Acquire);
        if original.is_null() {
            return None;
        }
        // SAFETY: every slot holds a function of the type its detour casts it to, and F is a function pointer of
        // the size of a pointer
        Some(unsafe { std::mem::transmute_copy::<*mut c_void, F>(&original) })
    }
}
