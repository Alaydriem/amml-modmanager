use std::cell::Cell;
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Area, Context, Id, Rect, ScrollArea, TextEdit};

use super::{FocusAction, FocusTarget};

/// A handler that shows one widget and applies the pending focus action to it once per pass.
pub struct FocusProbeHandler {
    target: FocusTarget,
    action: Rc<Cell<FocusAction>>,
    text: String,
}

impl FocusProbeHandler {
    const WIDGET: &'static str = "focus_probe_widget";
    const SCROLL_HEIGHT: f32 = 20.0;
    const SPACER_HEIGHT: f32 = 200.0;

    pub fn new(target: FocusTarget) -> (Self, Rc<Cell<FocusAction>>) {
        let action = Rc::new(Cell::new(FocusAction::None));
        (Self { target, action: Rc::clone(&action), text: String::new() }, action)
    }
}

impl FrameHandler for FocusProbeHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        let id = Id::unique(Self::WIDGET);
        let action = self.action.replace(FocusAction::None);
        Area::new(Id::unique("focus_probe_area")).show(ctx, |ui| {
            let response = match self.target {
                FocusTarget::TextField => ui.add(TextEdit::singleline(&mut self.text).id(id)),
                FocusTarget::Button => ui.add(egui::Button::new("go").sense(egui::Sense::click())),
                FocusTarget::ScrolledOutTextField => {
                    let text = &mut self.text;
                    ScrollArea::vertical()
                        .max_height(Self::SCROLL_HEIGHT)
                        .show(ui, |ui| {
                            ui.add_space(Self::SPACER_HEIGHT);
                            ui.add(TextEdit::singleline(text).id(id))
                        })
                        .inner
                }
            };
            let id = response.id;
            match action {
                FocusAction::None => {}
                FocusAction::Request => ctx.memory_mut(|m| m.request_focus(id)),
                FocusAction::Surrender => ctx.memory_mut(|m| m.surrender_focus(id)),
            }
        });
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
