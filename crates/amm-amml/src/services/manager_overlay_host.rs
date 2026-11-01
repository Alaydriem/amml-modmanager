#![allow(unsafe_code)]

use std::cell::UnsafeCell;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, Ordering};

use amm_abi::types::AmmStatus;
use amm_ffi::host::{
    amm_host_capture_buffer, amm_host_capture_done, amm_host_frame, amm_host_render_size, amm_host_stream_buffer,
    amm_host_stream_done, amm_host_textures_lost,
};
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};
use amm_overlay::OverlayHost;
use amml_sdk::ModRuntime;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

use super::ManagerState;

/// The manager host as the overlay sees it: the host frame, the textures-lost signal and the captures, through
/// the `amm_host_*` calls, and the cursor and HUD state each frame asks for, published for the game thread.
pub struct ManagerOverlayHost {
    state: &'static ManagerState,
    // the last host frame; written only by `frame`, which the overlay calls on the render thread, one at a time
    frame: UnsafeCell<AmmHostFrame>,
    // a failed host frame was logged; cleared by the next frame that works
    failure_logged: AtomicBool,
}

// SAFETY: `frame` is touched only by `OverlayHost::frame`, which the overlay calls inside a Present under its
// render lock, so never on two threads at once; the rest is atomics and a shared reference to atomics
unsafe impl Sync for ManagerOverlayHost {}

impl ManagerOverlayHost {
    pub fn new(state: &'static ManagerState) -> Self {
        Self { state, frame: UnsafeCell::new(AmmHostFrame::empty()), failure_logged: AtomicBool::new(false) }
    }

    /// A host that lives for the process, as the overlay requires.
    pub fn leaked(state: &'static ManagerState) -> &'static Self {
        Box::leak(Box::new(Self::new(state)))
    }
}

// SAFETY: `frame` returns the frame `amm_host_frame` wrote, whose layer and mesh pointers the host keeps valid
// until its next `amm_host_frame`, which only the next `frame` call makes. `capture_buffer` returns the buffer of
// `amm_host_capture_buffer`, writable for `height` rows of the returned pitch until `amm_host_capture_done`,
// `amm_host_textures_lost` or the next `amm_host_frame`; `stream_buffer` likewise for `amm_host_stream_buffer` and
// `amm_host_stream_done`
unsafe impl OverlayHost for ManagerOverlayHost {
    fn game_ready(&self) -> bool {
        self.state.tick().game_ready()
    }

    fn frame(&self, input: &AmmHostFrameInput) -> Option<&AmmHostFrame> {
        if ModRuntime::is_disabled() {
            return None;
        }
        let input = AmmHostFrameInput { menu_screen: self.state.menu().current(), ..*input };
        let out = self.frame.get();
        // SAFETY: `input` lives for the call, and `out` points to this host's frame cell, which no other call uses
        // while this one runs
        let status = unsafe { amm_host_frame(&input, out) };
        if status != AmmStatus::Ok {
            if !self.failure_logged.swap(true, Ordering::Relaxed) {
                curia::warn!("host frame failed, no layers drawn until it works again", { status: status as i32 });
            }
            return None;
        }
        self.failure_logged.store(false, Ordering::Relaxed);
        // SAFETY: `amm_host_frame` wrote the cell; the reference is used until the next `frame` call
        let frame = unsafe { &*out };
        self.state.frame().publish(self.state.latch(), frame.cursor_free, frame.cursor, frame.hide_vanilla_hud);
        Some(frame)
    }

    fn textures_lost(&self) {
        amm_host_textures_lost();
    }

    fn capture_buffer(&self, sequence: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)> {
        let mut row_pitch = 0u32;
        // SAFETY: `row_pitch` is a writable local
        let buffer = unsafe { amm_host_capture_buffer(sequence, width, height, &mut row_pitch) };
        NonNull::new(buffer).map(|buffer| (buffer, row_pitch))
    }

    fn capture_done(&self, sequence: u32, ok: bool, format: DXGI_FORMAT) {
        amm_host_capture_done(sequence, u8::from(ok), format.0 as u32);
    }

    fn stream_buffer(&self, id: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)> {
        let mut row_pitch = 0u32;
        // SAFETY: `row_pitch` is a writable local
        let buffer = unsafe { amm_host_stream_buffer(id, width, height, &mut row_pitch) };
        NonNull::new(buffer).map(|buffer| (buffer, row_pitch))
    }

    fn stream_done(&self, id: u32, ok: bool, format: DXGI_FORMAT) {
        let status = amm_host_stream_done(id, u8::from(ok), format.0 as u32);
        if status != AmmStatus::Ok {
            curia::warn!("stream frame report refused", { id: id, status: status as i32 });
        }
    }

    fn render_size(&self, width: u32, height: u32) -> Option<(u32, u32)> {
        if ModRuntime::is_disabled() {
            return None;
        }
        let (mut forced_width, mut forced_height) = (0u32, 0u32);
        // SAFETY: both out pointers are writable locals
        unsafe { amm_host_render_size(width, height, &mut forced_width, &mut forced_height) }
            .then_some((forced_width, forced_height))
    }

    fn size_forced(&self) -> bool {
        self.render_size(0, 0).is_some()
    }

    fn frame_skipped(&self) {
        self.state.frame().publish_safe(self.state.latch());
    }
}
