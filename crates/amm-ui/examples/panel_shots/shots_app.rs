use amm_core::structs::frame::MenuScreen;
use amm_ui::model::{PanelController, PanelIntent, PanelModel};
use amm_ui::runtime::{IconTextures, PanelLocale};
use amm_ui::views::PanelLayout;
use amml_ui::font::FontSource;
use amml_ui::text::UiLocale;
use amml_ui::theme::Theme;
use egui::{Context, Ui, ViewportCommand};

use crate::game_backdrop::GameBackdrop;
use crate::scene::Scene;
use crate::shot::Shot;
use crate::shot_runner::ShotRunner;

// Draws the manager panel over a stand-in game frame, one shot after another
pub struct ShotsApp {
    scene: Scene,
    shot: usize,
    controller: PanelController,
    intents: Vec<PanelIntent>,
    icons: IconTextures,
    runner: Option<ShotRunner>,
}

impl ShotsApp {
    const MANAGER_VERSION: &str = "1.4.1";

    /// `locale` is a language code, as the manager stores it: empty for English.
    pub fn new(ctx: &Context, mut scene: Scene, runner: Option<ShotRunner>, first: usize, locale: &str) -> Self {
        Theme::install(ctx, FontSource::Embedded);
        UiLocale::set(ctx, PanelLocale::localizer(locale));
        let shot = first.min(Shot::ALL.len() - 1);
        Self::stage(&mut scene, Shot::ALL[shot]);
        let controller = Shot::ALL[shot].controller(&scene);
        Self { scene, shot, controller, intents: Vec::new(), icons: IconTextures::new(), runner }
    }

    // a menu shot closes the panel on the start screen; every other shot has the panel open
    fn stage(scene: &mut Scene, shot: Shot) {
        scene.manager.set_menu_screen(if shot.is_menu() { MenuScreen::Start } else { MenuScreen::None });
        scene.manager.set_panel_open(!shot.is_menu());
    }

    fn advance(&mut self, ctx: &Context) {
        self.shot += 1;
        match Shot::ALL.get(self.shot) {
            Some(next) => {
                Self::stage(&mut self.scene, *next);
                self.controller = next.controller(&self.scene);
            }
            None => ctx.send_viewport_cmd(ViewportCommand::Close),
        }
    }

    // keeps a --view run responsive: page, tab and rebind changes go to the controller
    fn apply_intents(&mut self) {
        for intent in self.intents.drain(..) {
            match intent {
                PanelIntent::Action(action) => {
                    self.controller.on_action(&action);
                }
                PanelIntent::ShowPage(page) => self.controller.show_page(page),
                PanelIntent::BeginRebind(target) => self.controller.begin_rebind(target),
            }
        }
    }
}

impl eframe::App for ShotsApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let Some(shot) = Shot::ALL.get(self.shot).copied() else {
            return;
        };
        GameBackdrop::paint(ui.painter(), ui.max_rect());
        if shot.is_menu() {
            GameBackdrop::paint_menu(ui.painter(), ui.max_rect());
        }
        let model = PanelModel::new(&self.scene.manager, Self::MANAGER_VERSION, shot.now_ms());
        self.icons.sync(ui.ctx(), &model);
        PanelLayout::show(ui, &model, &self.controller, &self.icons, &mut self.intents);
        self.apply_intents();
        let ctx = ui.ctx().clone();
        let saved = self.runner.as_mut().is_some_and(|runner| runner.tick(&ctx, shot.file_name()));
        if saved {
            self.advance(&ctx);
        }
    }
}
