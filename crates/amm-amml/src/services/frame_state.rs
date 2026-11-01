use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use amm_abi::types::AmmCursorShape;

use super::StopLatch;

/// The cursor and HUD state the last host frame asked for, written on the render thread and read by the game
/// thread and the window thread. When frames stop running, the safe state (cursor grabbed, HUD shown) is
/// published once, so the game gets its mouse and HUD back once and not on every Present.
pub struct FrameState {
    cursor_free: AtomicBool,
    cursor: AtomicU8,
    hide_hud: AtomicBool,
    // the safe state was published because no frame ran; cleared by the next frame that ran
    safe_published: AtomicBool,
}

impl FrameState {
    pub const fn new() -> Self {
        Self {
            cursor_free: AtomicBool::new(false),
            cursor: AtomicU8::new(AmmCursorShape::Default as u8),
            hide_hud: AtomicBool::new(false),
            safe_published: AtomicBool::new(false),
        }
    }

    /// The state of a host frame that ran. Nothing changes while `latch` is raised.
    pub fn publish(&self, latch: &StopLatch, cursor_free: bool, cursor: AmmCursorShape, hide_hud: bool) {
        if latch.raised() {
            return;
        }
        self.safe_published.store(false, Ordering::Relaxed);
        self.store(cursor_free, cursor, hide_hud);
    }

    /// The safe state, once after frames stop running. True when it was published by this call; false while
    /// `latch` is raised or when it already was.
    pub fn publish_safe(&self, latch: &StopLatch) -> bool {
        if latch.raised() || self.safe_published.swap(true, Ordering::Relaxed) {
            return false;
        }
        self.store(false, AmmCursorShape::Default, false);
        true
    }

    pub fn cursor_free(&self) -> bool {
        self.cursor_free.load(Ordering::Relaxed)
    }

    pub fn cursor(&self) -> AmmCursorShape {
        match self.cursor.load(Ordering::Relaxed) {
            1 => AmmCursorShape::Arrow,
            2 => AmmCursorShape::Hand,
            3 => AmmCursorShape::Text,
            _ => AmmCursorShape::Default,
        }
    }

    pub fn hide_hud(&self) -> bool {
        self.hide_hud.load(Ordering::Relaxed)
    }

    fn store(&self, cursor_free: bool, cursor: AmmCursorShape, hide_hud: bool) {
        self.cursor_free.store(cursor_free, Ordering::Relaxed);
        self.cursor.store(cursor as u8, Ordering::Relaxed);
        self.hide_hud.store(hide_hud, Ordering::Relaxed);
    }
}

impl Default for FrameState {
    fn default() -> Self {
        Self::new()
    }
}
