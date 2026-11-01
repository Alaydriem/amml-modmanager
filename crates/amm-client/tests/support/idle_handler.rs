use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Context, Rect};

/// A handler that draws nothing.
pub struct IdleHandler;

impl FrameHandler for IdleHandler {
    fn frame(&mut self, _: &Context, _: &FrameInputView) -> FrameReply {
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
