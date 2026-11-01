use std::cell::{Cell, RefCell};
use std::rc::Rc;

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::{CursorShape, LayerKind};
use egui::{Area, Button, Context, Event, Id, Modifiers, PointerButton, Pos2, RawInput, Rect, Sense, pos2, vec2};

/// What `ClickHookHandler::adjust_input` injects on the next pass.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ClickStep {
    Idle,
    Press,
    Release,
}

/// What the handler saw: whether its button reported a click, and the layer `adjust_input` was called for.
#[derive(Default)]
pub struct ClickSeen {
    pub clicked: Cell<bool>,
    pub layer: RefCell<Option<LayerKind>>,
}

/// A handler with one button at a fixed spot. `adjust_input` adds the pointer events for the pending step.
pub struct ClickHookHandler {
    step: Rc<Cell<ClickStep>>,
    seen: Rc<ClickSeen>,
}

impl ClickHookHandler {
    const ORIGIN: Pos2 = pos2(100.0, 100.0);

    pub fn new() -> (Self, Rc<Cell<ClickStep>>, Rc<ClickSeen>) {
        let step = Rc::new(Cell::new(ClickStep::Idle));
        let seen = Rc::new(ClickSeen::default());
        (Self { step: Rc::clone(&step), seen: Rc::clone(&seen) }, step, seen)
    }

    fn button_centre() -> Pos2 {
        Rect::from_min_size(Self::ORIGIN, vec2(20.0, 20.0)).center()
    }
}

impl FrameHandler for ClickHookHandler {
    fn adjust_input(&mut self, layer: LayerKind, raw: &mut RawInput) {
        *self.seen.layer.borrow_mut() = Some(layer);
        let pressed = match self.step.replace(ClickStep::Idle) {
            ClickStep::Idle => return,
            ClickStep::Press => true,
            ClickStep::Release => false,
        };
        let pos = Self::button_centre();
        raw.events.push(Event::PointerMoved(pos));
        raw.events.push(Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        });
    }

    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        Area::new(Id::unique("click_hook_area")).fixed_pos(Self::ORIGIN).show(ctx, |ui| {
            let response = ui.add(Button::new("go").min_size(vec2(20.0, 20.0)).sense(Sense::click()));
            if response.clicked() {
                self.seen.clicked.set(true);
            }
        });
        FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING }
    }
}
