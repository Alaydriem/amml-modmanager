#![allow(unsafe_code)]

use amm_abi::types::AmmCursorShape;
use amm_ffi::host::{
    amm_host_char, amm_host_focus_lost, amm_host_pointer, amm_host_window_changed, amm_host_window_size,
};
use amm_ffi::types::AmmHostPointerKind;
use amm_overlay::InputSink;
use amml_sdk::ModRuntime;

use super::ManagerState;

/// The game window's input, from the overlay's window procedure, to the manager host. The cursor comes from the
/// last host frame, read without a lock on every `WM_SETCURSOR`.
pub struct ManagerInputSink {
    state: &'static ManagerState,
}

impl ManagerInputSink {
    pub fn new(state: &'static ManagerState) -> Self {
        Self { state }
    }

    /// A sink that lives for the process, as the overlay requires.
    pub fn leaked(state: &'static ManagerState) -> &'static Self {
        Box::leak(Box::new(Self::new(state)))
    }
}

impl InputSink for ManagerInputSink {
    fn pointer(&self, kind: AmmHostPointerKind, x: f32, y: f32, button: u8, down: bool, wheel: f32) -> bool {
        !ModRuntime::is_disabled() && amm_host_pointer(kind as u8, x, y, button, down, wheel).consume
    }

    fn char(&self, codepoint: u32) -> bool {
        !ModRuntime::is_disabled() && amm_host_char(codepoint).consume
    }

    fn focus_lost(&self) -> bool {
        !ModRuntime::is_disabled() && amm_host_focus_lost()
    }

    fn window_size(&self, minimised: bool, width: u32, height: u32) -> Option<(u32, u32)> {
        if ModRuntime::is_disabled() {
            return None;
        }
        let (mut sized_width, mut sized_height) = (0u32, 0u32);
        // SAFETY: both out pointers are writable locals
        unsafe { amm_host_window_size(minimised, width, height, &mut sized_width, &mut sized_height) }
            .then_some((sized_width, sized_height))
    }

    fn window_changed(&self) {
        if !ModRuntime::is_disabled() {
            amm_host_window_changed();
        }
    }

    fn cursor(&self) -> (bool, AmmCursorShape) {
        let frame = self.state.frame();
        (frame.cursor_free(), frame.cursor())
    }
}
