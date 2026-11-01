use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Context, Rect};

/// A handler that draws nothing and sets its flag when it is dropped, so a test sees whether the link freed its
/// frame state.
pub struct DropFlagHandler {
    dropped: Arc<AtomicBool>,
}

impl DropFlagHandler {
    pub fn new(dropped: Arc<AtomicBool>) -> Self {
        Self { dropped }
    }
}

impl FrameHandler for DropFlagHandler {
    fn frame(&mut self, _: &Context, _: &FrameInputView) -> FrameReply {
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}

impl Drop for DropFlagHandler {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::SeqCst);
    }
}
