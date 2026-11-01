use std::cell::RefCell;
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::{CursorShape, LayerKind};
use egui::{Color32, Context, LayerId, Rect, pos2, vec2};

use super::LayerSeen;

/// What each layer's context saw, shared between a probe handler and its test.
pub type SeenByLayer = Rc<RefCell<[LayerSeen; LayerKind::COUNT]>>;

/// A handler that paints one rectangle and records, per layer, what that layer's context saw.
pub struct LayerProbeHandler {
    seen: SeenByLayer,
}

impl LayerProbeHandler {
    pub fn new() -> (Self, SeenByLayer) {
        let seen = Rc::new(RefCell::new([LayerSeen::default(); LayerKind::COUNT]));
        (Self { seen: Rc::clone(&seen) }, seen)
    }
}

impl FrameHandler for LayerProbeHandler {
    fn frame(&mut self, ctx: &Context, input: &FrameInputView) -> FrameReply {
        let rect = Rect::from_min_size(pos2(4.0, 4.0), vec2(8.0, 8.0));
        ctx.layer_painter(LayerId::background()).rect_filled(rect, 0.0, Color32::RED);
        let (pointer, primary_down) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.primary_down()));
        let slot = &mut self.seen.borrow_mut()[input.layer().slot()];
        *slot = LayerSeen { pointer, primary_down, passes: slot.passes + 1 };
        FrameReply { cursor: CursorShape::Default, layer_rect: rect }
    }
}
