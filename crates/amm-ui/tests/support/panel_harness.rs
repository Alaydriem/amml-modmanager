use amm_core::structs::registry::ModIndex;
use amm_ui::model::{PanelAction, PanelController, PanelIntent, PanelModel, PanelPage, PanelTab};
use amm_ui::runtime::{IconTextures, PanelLocale};
use amm_ui::views::PanelLayout;
use amml_ui::font::FontSource;
use amml_ui::text::UiLocale;
use amml_ui::theme::Theme;
use egui::epaint::TextShape;
use egui::{Color32, Context, Event, Key, Modifiers, PointerButton, Pos2, RawInput, Rect, Shape, vec2};

use crate::support::FourMods;

// The panel on a screen (1920 by 1080 unless given), run one egui pass at a time, with what each
// pass painted and the actions it asked for. The controller takes the intents as the runtime does.
// Its limits: it calls `PanelLayout::show`, not `PanelRuntime::run`, so the runtime's key filtering
// (Escape, a rebind's chord) is not in it; it keeps no clip rects, so a test checks painted rects;
// and it never applies an action to the manager. `tests/runtime/` covers the runtime path.
pub struct PanelHarness {
    pub mods: FourMods,
    pub controller: PanelController,
    pub actions: Vec<PanelAction>,
    pub screen: Rect,
    ctx: Context,
    icons: IconTextures,
    time: f64,
    shapes: Vec<Shape>,
}

impl PanelHarness {
    pub const SCREEN: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1920.0, 1080.0));
    // the panel shots' window: the panel is smaller, so long text has less room
    pub const SMALL_SCREEN: Rect = Rect::from_min_max(Pos2::ZERO, Pos2::new(1280.0, 800.0));

    // the panel open, in the language `code`
    pub fn new(mods: FourMods, code: &str) -> Self {
        Self::on_screen(mods, code, Self::SCREEN)
    }

    // the panel open on `screen`, in the language `code`, with the fonts the game loads
    pub fn on_screen(mut mods: FourMods, code: &str, screen: Rect) -> Self {
        mods.manager.set_panel_open(true);
        let ctx = Context::default();
        Theme::install(&ctx, FontSource::Embedded);
        UiLocale::set(&ctx, PanelLocale::localizer(code));
        let controller = PanelController::new();
        let mut harness = Self {
            mods,
            controller,
            actions: Vec::new(),
            screen,
            ctx,
            icons: IconTextures::new(),
            time: 0.0,
            shapes: Vec::new(),
        };
        harness.run();
        harness
    }

    // shows tab `tab` of the mod in slot `index`
    pub fn open(&mut self, index: ModIndex, tab: PanelTab) {
        self.controller.on_action(&PanelAction::Select(Some(index)));
        self.controller.on_action(&PanelAction::SelectTab(tab));
        self.run();
    }

    // shows a page of the Manager group
    pub fn show_page(&mut self, page: PanelPage) {
        self.controller.show_page(page);
        self.run();
    }

    // how many painted texts equal `text`
    pub fn count(&self, text: &str) -> usize {
        self.texts().iter().filter(|(painted, _)| painted == text).count()
    }

    // two passes, so sizes measured in the first are used in the second
    pub fn run(&mut self) {
        self.frame(Vec::new());
        self.frame(Vec::new());
    }

    pub fn frame(&mut self, events: Vec<Event>) {
        self.time += 1.0 / 60.0;
        let raw = RawInput { screen_rect: Some(self.screen), time: Some(self.time), events, ..RawInput::default() };
        let mut intents = Vec::new();
        let (manager, controller) = (&self.mods.manager, &self.controller);
        let model = PanelModel::new(manager, "1.4.1", 0);
        self.icons.sync(&self.ctx, &model);
        let icons = &self.icons;
        let mut output = self.ctx.run_ui(raw, |ui| {
            PanelLayout::show(ui, &model, controller, icons, &mut intents);
        });
        // no texture leaves this test; the font atlas is not uploaded anywhere
        output.textures_delta.clear();
        self.shapes = output.shapes.into_iter().map(|clipped| clipped.shape).collect();
        for intent in intents {
            match intent {
                PanelIntent::Action(action) => {
                    if let Some(action) = self.controller.on_action(&action) {
                        self.actions.push(action);
                    }
                }
                PanelIntent::ShowPage(page) => self.controller.show_page(page),
                PanelIntent::BeginRebind(target) => self.controller.begin_rebind(target),
            }
        }
    }

    // a press and a release at `at`, then a pass to settle
    pub fn click(&mut self, at: Pos2) {
        self.frame(vec![Event::PointerMoved(at)]);
        for pressed in [true, false] {
            let button = PointerButton::Primary;
            self.frame(vec![Event::PointerButton { pos: at, button, pressed, modifiers: Modifiers::NONE }]);
        }
        self.frame(Vec::new());
    }

    // types `text` into the focused field, then presses Enter
    pub fn type_and_enter(&mut self, text: &str) {
        self.frame(vec![Event::Text(text.to_owned())]);
        for pressed in [true, false] {
            let modifiers = Modifiers::NONE;
            self.frame(vec![Event::Key { key: Key::Enter, physical_key: None, pressed, repeat: false, modifiers }]);
        }
        self.frame(Vec::new());
    }

    // every text the last pass painted, with its rect
    pub fn texts(&self) -> Vec<(String, Rect)> {
        let mut out = Vec::new();
        Self::collect_texts(&self.shapes, &mut out);
        out
    }

    // the rect of the first painted text equal to `text`
    pub fn text_rect(&self, text: &str) -> Option<Rect> {
        self.texts().into_iter().find(|(painted, _)| painted == text).map(|(_, rect)| rect)
    }

    // true when the first painted text equal to `text` is drawn faded, by its opacity or its colour
    pub fn text_faded(&self, text: &str) -> Option<bool> {
        let mut found = None;
        Self::visit_texts(&self.shapes, &mut |shape| {
            if found.is_none() && shape.galley.text() == text {
                // egui fades a shape by its colours: the fallback and each glyph vertex
                let glyph = shape.galley.rows.iter().flat_map(|row| row.visuals.mesh.vertices.iter()).next();
                let colour = glyph.map_or(shape.fallback_color, |vertex| vertex.color);
                found = Some(shape.opacity_factor < 1.0 || colour.a() < u8::MAX);
            }
        });
        found
    }

    // the textured rects the last pass painted
    pub fn image_rects(&self) -> Vec<Rect> {
        let mut out = Vec::new();
        Self::collect_images(&self.shapes, &mut out);
        out
    }

    // the filled rects of exactly this size and colour the last pass painted
    pub fn filled_rects(&self, size: f32, colour: Color32) -> Vec<Rect> {
        let mut out = Vec::new();
        Self::collect_rects(&self.shapes, &mut out);
        out.into_iter()
            .filter(|(rect, fill)| *fill == colour && (rect.size() - vec2(size, size)).length() < 0.01)
            .map(|(rect, _)| rect)
            .collect()
    }

    fn collect_texts(shapes: &[Shape], out: &mut Vec<(String, Rect)>) {
        Self::visit_texts(shapes, &mut |text| {
            out.push((text.galley.text().to_owned(), text.galley.rect.translate(text.pos.to_vec2())));
        });
    }

    fn visit_texts(shapes: &[Shape], visit: &mut impl FnMut(&TextShape)) {
        for shape in shapes {
            match shape {
                Shape::Text(text) => visit(text),
                Shape::Vec(inner) => Self::visit_texts(inner, visit),
                _ => {}
            }
        }
    }

    fn collect_images(shapes: &[Shape], out: &mut Vec<Rect>) {
        for shape in shapes {
            match shape {
                Shape::Rect(rect) if rect.brush.is_some() => out.push(rect.rect),
                Shape::Vec(inner) => Self::collect_images(inner, out),
                _ => {}
            }
        }
    }

    fn collect_rects(shapes: &[Shape], out: &mut Vec<(Rect, Color32)>) {
        for shape in shapes {
            match shape {
                Shape::Rect(rect) => out.push((rect.rect, rect.fill)),
                Shape::Vec(inner) => Self::collect_rects(inner, out),
                _ => {}
            }
        }
    }
}
