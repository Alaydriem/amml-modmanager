use std::cell::RefCell;
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Context, Rect, Vec2};

/// The event count and screen size of the last pass's raw input, shared with the test.
pub type RawInputSeen = Rc<RefCell<Option<(usize, Vec2)>>>;

/// A handler that keeps the event count and screen size of the raw input of each pass. It uses the default
/// `adjust_input`.
pub struct RawInputProbeHandler {
    seen: RawInputSeen,
}

impl RawInputProbeHandler {
    pub fn new() -> (Self, RawInputSeen) {
        let seen = Rc::new(RefCell::new(None));
        (Self { seen: Rc::clone(&seen) }, seen)
    }
}

impl FrameHandler for RawInputProbeHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        let (events, size) = ctx.input(|i| (i.raw.events.len(), i.content_rect().size()));
        *self.seen.borrow_mut() = Some((events, size));
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
