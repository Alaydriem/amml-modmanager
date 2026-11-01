use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Area, Context, Id, Rect, TextEdit};

use super::FocusAction;

/// A handler that shows one text field over shared text and applies the pending focus action to it once per pass.
pub struct ClipboardProbeHandler {
    action: Rc<Cell<FocusAction>>,
    text: Rc<RefCell<String>>,
}

impl ClipboardProbeHandler {
    const FIELD: &'static str = "clipboard_probe_field";

    pub fn new(text: &str) -> (Self, Rc<Cell<FocusAction>>, Rc<RefCell<String>>) {
        let action = Rc::new(Cell::new(FocusAction::None));
        let text = Rc::new(RefCell::new(text.to_owned()));
        (Self { action: Rc::clone(&action), text: Rc::clone(&text) }, action, text)
    }
}

impl FrameHandler for ClipboardProbeHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        let action = self.action.replace(FocusAction::None);
        Area::new(Id::unique("clipboard_probe_area")).show(ctx, |ui| {
            let mut text = self.text.borrow_mut();
            let id = ui.add(TextEdit::singleline(&mut *text).id(Id::unique(Self::FIELD))).id;
            match action {
                FocusAction::None => {}
                FocusAction::Request => ctx.memory_mut(|m| m.request_focus(id)),
                FocusAction::Surrender => ctx.memory_mut(|m| m.surrender_focus(id)),
            }
        });
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
