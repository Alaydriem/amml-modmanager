use std::cell::Cell;
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::{CursorShape, LayerKind};
use egui::{Context, Rect};

/// A handler that paints nothing and panics on a call for the overlay layer while its switch is on.
pub struct OverlayPanickingHandler {
    armed: Rc<Cell<bool>>,
}

impl OverlayPanickingHandler {
    pub fn new() -> (Self, Rc<Cell<bool>>) {
        let armed = Rc::new(Cell::new(false));
        (Self { armed: Rc::clone(&armed) }, armed)
    }
}

impl FrameHandler for OverlayPanickingHandler {
    fn frame(&mut self, _: &Context, input: &FrameInputView) -> FrameReply {
        if self.armed.get() && input.layer() == LayerKind::Overlay {
            panic!("overlay failed on purpose");
        }
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
