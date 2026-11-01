use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::{CursorShape, LayerKind};
use egui::{Align2, Area, Context, Frame, Id, Rect, RichText, Sense, Window, pos2, vec2};

/// A mod frame handler whose egui pass is about the size of the demo mod's: on the Hud layer a card with labels,
/// two rows of marks and a bar, on the Overlay layer a window with labels, a checkbox and a button.
#[derive(Default)]
pub struct DemoSizedHandler {
    checked: bool,
}

impl DemoSizedHandler {
    const MARKS_PER_ROW: usize = 16;
    const MARK: f32 = 8.0;
    const MARK_GAP: f32 = 3.0;
    const BAR: (f32, f32) = (150.0, 6.0);

    pub fn new() -> Self {
        Self::default()
    }

    fn card(ctx: &Context) -> Rect {
        let shown = Area::new(Id::unique("bench.card")).anchor(Align2::LEFT_TOP, vec2(8.0, 8.0)).show(ctx, |ui| {
            Frame::popup(ui.style()).show(ui, |ui| {
                ui.label(RichText::new("Demo").strong());
                ui.label("Rows 32");
                ui.label("Opacity 0.80");
                for _ in 0..2 {
                    let width = Self::MARKS_PER_ROW as f32 * (Self::MARK + Self::MARK_GAP);
                    let (rect, _) = ui.allocate_exact_size(vec2(width, Self::MARK), Sense::hover());
                    for i in 0..Self::MARKS_PER_ROW {
                        let x = rect.left() + i as f32 * (Self::MARK + Self::MARK_GAP);
                        let mark = Rect::from_min_size(pos2(x, rect.top()), vec2(Self::MARK, Self::MARK));
                        ui.painter().rect_filled(mark, 2.0, egui::Color32::LIGHT_BLUE);
                    }
                }
                let (rect, _) = ui.allocate_exact_size(vec2(Self::BAR.0, Self::BAR.1), Sense::hover());
                ui.painter().rect_filled(rect, 3.0, egui::Color32::DARK_GRAY);
                ui.label("Camera idle");
                ui.label("Last event none");
            });
        });
        shown.response.rect
    }

    fn window(&mut self, ctx: &Context) -> Rect {
        let shown = Window::new("Demo window")
            .collapsible(false)
            .resizable(false)
            .default_pos(pos2(120.0, 120.0))
            .show(ctx, |ui| {
                ui.label("This window holds the Overlay lease and asks for the cursor.");
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.checked, "Show the last event on the card");
                });
                let _ = ui.button("Close");
            });
        shown.map_or(Rect::NOTHING, |window| window.response.rect)
    }
}

impl FrameHandler for DemoSizedHandler {
    fn frame(&mut self, ctx: &Context, input: &FrameInputView) -> FrameReply {
        match input.layer() {
            LayerKind::Hud => FrameReply { cursor: CursorShape::Default, layer_rect: Self::card(ctx) },
            LayerKind::Overlay => FrameReply { cursor: CursorShape::Arrow, layer_rect: self.window(ctx) },
        }
    }
}
