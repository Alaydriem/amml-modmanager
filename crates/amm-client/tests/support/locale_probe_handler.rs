use std::cell::RefCell;
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use amml_i18n::text::Text;
use amml_ui::AmmlUiMessage;
use amml_ui::text::UiText;
use egui::{Context, Rect};

/// A handler that keeps, on each pass, the text an amml-ui widget would show for its Close word.
pub struct LocaleProbeHandler {
    seen: Rc<RefCell<String>>,
}

impl LocaleProbeHandler {
    pub fn new() -> (Self, Rc<RefCell<String>>) {
        let seen = Rc::new(RefCell::new(String::new()));
        (Self { seen: Rc::clone(&seen) }, seen)
    }
}

impl FrameHandler for LocaleProbeHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        *self.seen.borrow_mut() = UiText::resolve(ctx, &Text::message(AmmlUiMessage::Close));
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
