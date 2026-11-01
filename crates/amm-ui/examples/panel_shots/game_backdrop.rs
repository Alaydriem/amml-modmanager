use egui::epaint::Mesh;
use egui::{Align2, Color32, FontId, Painter, Rect, Shape, Stroke, StrokeKind, pos2, vec2};

// A stand-in for the game frame behind the overlay: a dusk sky, blocky hills, and a hotbar. Its
// colours stand for a game picture; they are not design values and do not come from tokens.
pub struct GameBackdrop;

impl GameBackdrop {
    const SKY_TOP: Color32 = Color32::from_rgb(0x1B, 0x2A, 0x45);
    const SKY_LOW: Color32 = Color32::from_rgb(0x3A, 0x4E, 0x5E);
    const HILL_FAR: Color32 = Color32::from_rgb(0x22, 0x3B, 0x33);
    const HILL_NEAR: Color32 = Color32::from_rgb(0x1A, 0x30, 0x22);
    const GROUND: Color32 = Color32::from_rgb(0x14, 0x24, 0x19);
    const HOTBAR: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 120);
    const SLOT: Color32 = Color32::from_rgba_premultiplied(40, 40, 40, 40);
    const SLOT_LINE: f32 = 2.0;
    const BLOCK: f32 = 24.0;
    const MENU_DIM: Color32 = Color32::from_rgba_premultiplied(0, 0, 0, 110);
    const VERSION: &str = "v1.26.0";
    const VERSION_SIZE: f32 = 18.0;
    const VERSION_MARGIN: f32 = 8.0;

    pub fn paint(painter: &Painter, rect: Rect) {
        let horizon = rect.top() + rect.height() * 0.58;
        Self::gradient(
            painter,
            Rect::from_min_max(rect.min, pos2(rect.right(), horizon + 80.0)),
            Self::SKY_TOP,
            Self::SKY_LOW,
        );
        Self::hills(painter, rect, horizon - 40.0, 7, Self::HILL_FAR, 3.0);
        Self::hills(painter, rect, horizon + 10.0, 5, Self::HILL_NEAR, 5.0);
        painter.rect_filled(Rect::from_min_max(pos2(rect.left(), horizon + 70.0), rect.max), 0.0, Self::GROUND);
        Self::hotbar(painter, rect);
    }

    // the game's pause screen over the frame: a dim layer and the version text in the bottom-right corner
    pub fn paint_menu(painter: &Painter, rect: Rect) {
        painter.rect_filled(rect, 0.0, Self::MENU_DIM);
        let corner = pos2(rect.right() - Self::VERSION_MARGIN, rect.bottom() - Self::VERSION_MARGIN);
        painter.text(
            corner,
            Align2::RIGHT_BOTTOM,
            Self::VERSION,
            FontId::monospace(Self::VERSION_SIZE),
            Color32::WHITE,
        );
    }

    fn gradient(painter: &Painter, rect: Rect, top: Color32, bottom: Color32) {
        let mut mesh = Mesh::default();
        mesh.colored_vertex(rect.left_top(), top);
        mesh.colored_vertex(rect.right_top(), top);
        mesh.colored_vertex(rect.left_bottom(), bottom);
        mesh.colored_vertex(rect.right_bottom(), bottom);
        mesh.add_triangle(0, 1, 2);
        mesh.add_triangle(1, 2, 3);
        painter.add(Shape::mesh(mesh));
    }

    // a stepped skyline of block columns; `seed` varies the heights
    fn hills(painter: &Painter, rect: Rect, base: f32, seed: u32, colour: Color32, amplitude: f32) {
        let columns = (rect.width() / Self::BLOCK).ceil() as u32 + 1;
        for column in 0..columns {
            let wave = ((column as f32 * 0.37 + seed as f32).sin() + (column as f32 * 0.11 + seed as f32 * 2.0).cos())
                * amplitude;
            let top = base - wave.round() * Self::BLOCK * 0.5;
            let x = rect.left() + column as f32 * Self::BLOCK;
            painter.rect_filled(Rect::from_min_max(pos2(x, top), pos2(x + Self::BLOCK, rect.bottom())), 0.0, colour);
        }
    }

    fn hotbar(painter: &Painter, rect: Rect) {
        let slot = 40.0;
        let bar =
            Rect::from_center_size(pos2(rect.center().x, rect.bottom() - 30.0), vec2(slot * 9.0 + 4.0, slot + 4.0));
        painter.rect_filled(bar, 0.0, Self::HOTBAR);
        for index in 0..9 {
            let cell =
                Rect::from_min_size(pos2(bar.left() + 2.0 + index as f32 * slot, bar.top() + 2.0), vec2(slot, slot))
                    .shrink(2.0);
            painter.rect_stroke(cell, 0.0, Stroke::new(Self::SLOT_LINE, Self::SLOT), StrokeKind::Inside);
        }
    }
}
