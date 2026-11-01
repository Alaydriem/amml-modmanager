use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Color32, Context, LayerId, Rect};

/// A handler that fills one rectangle, given in points, and reports it as its layer.
pub struct RectHandler {
    pub rect: Rect,
}

impl FrameHandler for RectHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        ctx.layer_painter(LayerId::background()).rect_filled(self.rect, 0.0, Color32::RED);
        FrameReply { cursor: CursorShape::Hand, layer_rect: self.rect }
    }
}
