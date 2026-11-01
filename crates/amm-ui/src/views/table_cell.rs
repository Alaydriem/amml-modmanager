use amml_i18n::text::Text;
use amml_ui::draw::{Draw, TextFit, TextStyle};
use amml_ui::state::InteractionState;
use amml_ui::text::UiText;
use amml_ui::widget::Chip;
use egui::{Color32, Painter, Rect, Response, Sense, Ui, pos2, vec2};

/// The text in one cell of an amml-ui table row: one line, or two lines (a name over its meaning, a
/// claim chip over its reason) centred on the row. Each line is cut to the cell's width.
pub struct TableCell;

impl TableCell {
    /// Paper: Mod manager screens 03 and 04, the gap between the two lines of a cell.
    const LINE_GAP: f32 = 2.0;

    pub fn text(ui: &mut Ui, text: &Text, style: TextStyle, colour: Color32) -> Response {
        let words = UiText::resolve(ui.ctx(), text);
        let width = ui.available_width().max(0.0);
        let galley = Draw::galley(ui.painter(), &words, style, colour, TextFit::Truncate(width));
        let (rect, response) = ui.allocate_exact_size(galley.size(), Sense::hover());
        ui.painter().galley(rect.min, galley, colour);
        response
    }

    pub fn text_over_text(ui: &mut Ui, top: (&Text, TextStyle, Color32), bottom: (&Text, TextStyle, Color32)) {
        let (text, style, colour) = top;
        let words = UiText::resolve(ui.ctx(), text);
        let galley = Draw::galley(ui.painter(), &words, style, colour, TextFit::Truncate(ui.available_width()));
        Self::over(ui, galley.size().y, |painter, rect| painter.galley(rect.min, galley, colour), bottom);
    }

    pub fn chip_over_text(ui: &mut Ui, chip: Chip, bottom: (&Text, TextStyle, Color32)) {
        let size = vec2(chip.desired_width(ui.ctx()).min(ui.available_width()), Chip::height());
        Self::over(
            ui,
            size.y,
            |painter, rect| chip.paint(painter, Rect::from_min_size(rect.min, size), InteractionState::Rest),
            bottom,
        );
    }

    // a top part `top_height` high over one line of text, the pair centred on the cell
    fn over(ui: &mut Ui, top_height: f32, top: impl FnOnce(&Painter, Rect), bottom: (&Text, TextStyle, Color32)) {
        let (text, style, colour) = bottom;
        let cell = ui.max_rect();
        let words = UiText::resolve(ui.ctx(), text);
        let galley = Draw::galley(ui.painter(), &words, style, colour, TextFit::Truncate(cell.width()));
        let height = top_height + Self::LINE_GAP + galley.size().y;
        let y = cell.center().y - height / 2.0;
        let painter = ui.painter().clone();
        top(&painter, Rect::from_min_size(pos2(cell.left(), y), vec2(cell.width(), top_height)));
        painter.galley(pos2(cell.left(), y + top_height + Self::LINE_GAP), galley, colour);
        ui.allocate_rect(Rect::from_min_size(pos2(cell.left(), y), vec2(cell.width(), height)), Sense::hover());
    }
}
