use std::sync::{Arc, Mutex};

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Context, FontFamily, Rect};

/// A handler that counts the contexts it was told about and records the font families of the last pass.
#[derive(Default)]
pub struct ContextProbeHandler {
    pub new_contexts: Arc<Mutex<usize>>,
    pub families: Arc<Mutex<Vec<FontFamily>>>,
}

impl FrameHandler for ContextProbeHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        *self.families.lock().unwrap_or_else(std::sync::PoisonError::into_inner) = ctx.fonts(|fonts| fonts.families());
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }

    fn new_context(&mut self, _: &Context) {
        *self.new_contexts.lock().unwrap_or_else(std::sync::PoisonError::into_inner) += 1;
    }
}
