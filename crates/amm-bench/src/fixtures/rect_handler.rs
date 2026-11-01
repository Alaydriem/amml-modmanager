use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Color32, Context, LayerId, Rect, pos2};

/// A mod frame handler that paints one rectangle.
pub struct RectHandler;

impl FrameHandler for RectHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        let rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(110.0, 60.0));
        ctx.layer_painter(LayerId::background()).rect_filled(rect, 0.0, Color32::RED);
        FrameReply { cursor: CursorShape::Default, layer_rect: rect }
    }
}
