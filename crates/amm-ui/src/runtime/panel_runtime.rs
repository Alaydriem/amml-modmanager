use amm_core::services::egui_bridge::{MeshFlattener, RawInputBuilder};
use amm_core::structs::frame::{FrameInputData, UiEvent, UiEventKind};
use amm_core::structs::input::Modifiers;
use amm_core::structs::mesh::MeshBuffers;
use std::sync::Arc;

use amml_i18n::localize::Localizer;
use amml_ui::font::FontSource;
use amml_ui::text::UiLocale;
use amml_ui::theme::Theme;
use egui::{Context, RawInput, Rect};

use crate::model::{PanelAction, PanelController, PanelIntent, PanelModel};
use crate::runtime::{IconTextures, PanelLocale};
use crate::views::PanelLayout;

/// The manager's own egui pass: the panel, the toasts and the menu button. It owns the
/// egui context, the panel state and the reused event, intent and action buffers. The host runs it
/// on each frame where the panel is open, a toast is visible or the menu button shows, draws `out`,
/// and applies the returned actions.
pub struct PanelRuntime {
    fonts: FontSource,
    // the panel's words in the manager's language; set again on every new context
    localizer: Arc<Localizer>,
    ctx: Context,
    builder: RawInputBuilder,
    controller: PanelController,
    // one texture per mod icon, uploaded into `ctx`
    icons: IconTextures,
    events: Vec<UiEvent>,
    intents: Vec<PanelIntent>,
    actions: Vec<PanelAction>,
    // the menu button's rect in physical pixels from the last pass that drew it
    menu_button: Option<[f32; 4]>,
}

impl PanelRuntime {
    const ESCAPE: u32 = 0x1B;

    /// `locale` is the manager's language code: empty for English, `en_XA` for the pseudo-locale.
    pub fn new(fonts: FontSource, locale: &str) -> Self {
        let localizer = PanelLocale::localizer(locale);
        Self {
            fonts,
            ctx: Self::context(fonts, &localizer),
            localizer,
            builder: RawInputBuilder::new(),
            controller: PanelController::new(),
            icons: IconTextures::new(),
            events: Vec::new(),
            intents: Vec::new(),
            actions: Vec::new(),
            menu_button: None,
        }
    }

    /// Runs one pass, fills `out` in physical pixels, and returns the actions for the host. The
    /// returned slice is reused by the next call.
    pub fn run(&mut self, input: &FrameInputData, model: &PanelModel, out: &mut MeshBuffers) -> &[PanelAction] {
        self.actions.clear();
        self.intents.clear();
        self.keys(input, model);
        let filtered = FrameInputData { events: &self.events, ..*input };
        let mut raw = RawInput::default();
        self.builder.build(&filtered, &mut raw);
        self.icons.sync(&self.ctx, model);
        let controller = &self.controller;
        let icons = &self.icons;
        let intents = &mut self.intents;
        let mut corner = None;
        let mut output = self.ctx.run_ui(raw, |ui| corner = PanelLayout::show(ui, model, controller, icons, intents));
        let ppp = output.pixels_per_point;
        let primitives = self.ctx.tessellate(std::mem::take(&mut output.shapes), ppp);
        let textures = self.ctx.tex_manager();
        MeshFlattener::flatten(&primitives, &output.textures_delta, &textures.read(), ppp, out);
        output.textures_delta.clear();
        self.menu_button = corner.map(|rect| Self::pixels(input, rect));
        self.apply_intents();
        &self.actions
    }

    /// Where the last pass drew the corner row (the Mods button and the menu entries) in physical pixels, or `None`
    /// when it drew none.
    pub fn menu_button_rect(&self) -> Option<[f32; 4]> {
        self.menu_button
    }

    /// The device dropped every texture: the next pass sends the full font atlas again.
    pub fn textures_lost(&mut self) {
        self.ctx = Self::context(self.fonts, &self.localizer);
        self.icons.clear();
    }

    /// The person picked another language: the next pass draws in it.
    pub fn set_locale(&mut self, code: &str) {
        self.localizer = PanelLocale::localizer(code);
        UiLocale::set(&self.ctx, Arc::clone(&self.localizer));
    }

    /// The panel state, for host commands that must follow the panel's rules, such as the toggle
    /// chord closing the panel through `on_action(&PanelAction::Close)`.
    pub fn controller_mut(&mut self) -> &mut PanelController {
        &mut self.controller
    }

    // a rect in points as physical pixels
    fn pixels(input: &FrameInputData, rect: Rect) -> [f32; 4] {
        let ppp = input.pixels_per_point.max(f32::EPSILON);
        [rect.min.x * ppp, rect.min.y * ppp, rect.max.x * ppp, rect.max.y * ppp]
    }

    fn context(fonts: FontSource, localizer: &Arc<Localizer>) -> Context {
        let ctx = Context::default();
        Theme::install(&ctx, fonts);
        UiLocale::set(&ctx, Arc::clone(localizer));
        ctx
    }

    // Takes the key events the panel itself handles out of the stream egui sees: every key while a
    // rebind waits for its chord, where Escape cancels it, and Escape, which closes.
    fn keys(&mut self, input: &FrameInputData, model: &PanelModel) {
        self.events.clear();
        let keyboard = input.input_flags & FrameInputData::ALLOW_KEYBOARD != 0;
        for event in input.events {
            let is_key = event.kind == UiEventKind::Key || event.kind == UiEventKind::Char;
            if !keyboard || !model.panel_open() || !is_key {
                self.events.push(*event);
                continue;
            }
            if self.controller.rebinding().is_some() {
                if event.kind == UiEventKind::Key {
                    let mods = Self::modifiers(event.modifiers);
                    if let Some(action) = self.controller.on_key(event.key, event.down, mods) {
                        self.actions.push(action);
                    }
                }
                continue;
            }
            if event.kind == UiEventKind::Key && event.key == Self::ESCAPE {
                if event.down {
                    self.push(&PanelAction::Close);
                }
                continue;
            }
            self.events.push(*event);
        }
    }

    fn apply_intents(&mut self) {
        let intents = std::mem::take(&mut self.intents);
        for intent in &intents {
            match intent {
                PanelIntent::Action(action) => self.push(action),
                PanelIntent::ShowPage(page) => self.controller.show_page(*page),
                PanelIntent::BeginRebind(target) => self.controller.begin_rebind(*target),
            }
        }
        self.intents = intents;
    }

    fn push(&mut self, action: &PanelAction) {
        if let Some(result) = self.controller.on_action(action) {
            self.actions.push(result);
        }
    }

    fn modifiers(mask: u8) -> Modifiers {
        Modifiers {
            ctrl: mask & UiEvent::MOD_CTRL != 0,
            shift: mask & UiEvent::MOD_SHIFT != 0,
            alt: mask & UiEvent::MOD_ALT != 0,
        }
    }
}
