use amml_ui::draw::{Corner, Draw};
use amml_ui::tokens::rule::Elevation;
use amml_ui::tokens::{ColorToken, SpacingToken, color, spacing};
use amml_ui::widget::{Surface, SurfaceLevel};
use egui::{Rect, Sense, Ui, UiBuilder, pos2, vec2};

use crate::model::{PanelController, PanelIntent, PanelModel, PanelPage};
use crate::runtime::IconTextures;
use crate::views::{
    ConflictsView, CornerRowView, HeaderView, KeybindsPage, ManagerSettingsView, ModDetailView, ModListView, ToastsView,
};

/// One pass of the manager's own UI over the whole screen: the panel when it is open, the toasts,
/// and the menu button while the panel is closed.
pub struct PanelLayout;

impl PanelLayout {
    /// Paper: Mod manager screen 01, the panel's size on a 1920 by 1080 screen.
    const PANEL_MAX: (f32, f32) = (1474.0, 900.0);
    const SCREEN_MARGIN: f32 = 32.0;
    /// Paper: Mod manager screen 01, the sidebar's width with its padding and its right border.
    const SIDEBAR: f32 = 470.0;
    /// Paper: Mod manager screen 01, the sidebar's padding at the top and bottom, the left and the right.
    const SIDEBAR_PAD: (f32, f32, f32) = (24.0, 24.0, 20.0);
    /// Paper: Mod manager screen 01, the detail pane's padding at the top, right, bottom and left.
    const PANE_PAD: (f32, f32, f32, f32) = (20.0, 24.0, 24.0, 28.0);

    /// Returns the rect of the corner row when it drew one, in points.
    pub fn show(
        ui: &mut Ui,
        model: &PanelModel,
        controller: &PanelController,
        icons: &IconTextures,
        out: &mut Vec<PanelIntent>,
    ) -> Option<Rect> {
        let screen = ui.max_rect();
        let mut panel = None;
        if model.panel_open() {
            Draw::fill(ui.painter(), screen, Corner::None, <color::BgOverlay as ColorToken>::VALUE);
            ui.allocate_rect(screen, Sense::click());
            Self::panel(ui, screen, model, controller, icons, out);
            panel = Some(Self::panel_rect(screen));
        }
        let corner = if model.menu_button_visible() { CornerRowView::show(ui, screen, model, out) } else { None };
        ToastsView::show(ui.ctx(), screen, panel, corner, model);
        corner
    }

    /// The panel's rect on a screen: centred, inside a 32 point margin, at most 1474 by 900.
    pub fn panel_rect(screen: Rect) -> Rect {
        let size = vec2(
            (screen.width() - 2.0 * Self::SCREEN_MARGIN).min(Self::PANEL_MAX.0),
            (screen.height() - 2.0 * Self::SCREEN_MARGIN).min(Self::PANEL_MAX.1),
        );
        Rect::from_center_size(screen.center(), size.max(egui::Vec2::ZERO))
    }

    /// The sidebar's content on a screen: under the header, inside the sidebar's padding.
    pub fn sidebar(screen: Rect) -> Rect {
        let panel = Self::panel_rect(screen);
        let (pad_y, pad_left, pad_right) = Self::SIDEBAR_PAD;
        let right = panel.left() + Self::SIDEBAR - <spacing::Divider as SpacingToken>::VALUE - pad_right;
        Rect::from_min_max(
            pos2(panel.left() + pad_left, panel.top() + HeaderView::height() + pad_y),
            pos2(right, panel.bottom() - pad_y),
        )
    }

    /// The right pane's content on a screen: right of the sidebar's divider, inside the pane's padding.
    pub fn pane(screen: Rect) -> Rect {
        let panel = Self::panel_rect(screen);
        let (top, right, bottom, left) = Self::PANE_PAD;
        Rect::from_min_max(
            pos2(panel.left() + Self::SIDEBAR + left, panel.top() + HeaderView::height() + top),
            pos2(panel.right() - right, panel.bottom() - bottom),
        )
    }

    /// The page shown: the operator's choice while it is valid, else the first mod, else Conflicts.
    pub fn page(model: &PanelModel, controller: &PanelController) -> PanelPage {
        match controller.page() {
            Some(PanelPage::Mod(index)) if model.registered(index).is_some() => PanelPage::Mod(index),
            Some(PanelPage::Mod(_)) | None => model.first_mod().map_or(PanelPage::Conflicts, PanelPage::Mod),
            Some(page) => page,
        }
    }

    fn panel(
        ui: &mut Ui,
        screen: Rect,
        model: &PanelModel,
        controller: &PanelController,
        icons: &IconTextures,
        out: &mut Vec<PanelIntent>,
    ) {
        let rect = Self::panel_rect(screen);
        let painter = ui.painter().clone();
        Surface::paint(&painter, rect, SurfaceLevel::Panel, Elevation::Floating);
        let header = Rect::from_min_size(rect.min, vec2(rect.width(), HeaderView::height()));
        HeaderView::show(&mut ui.new_child(UiBuilder::new().max_rect(header)), model, out);
        // the header bar draws its own border; the panel's edge goes back over it
        Draw::stroke(&painter, rect, Corner::Panel, <color::LineStrong as ColorToken>::VALUE);
        let line = <spacing::Divider as SpacingToken>::VALUE;
        let divider_x = rect.left() + Self::SIDEBAR - line;
        let divider =
            Rect::from_min_max(pos2(divider_x, header.bottom()), pos2(divider_x + line, rect.bottom() - line));
        Draw::fill(&painter, divider, Corner::None, <color::Line as ColorToken>::VALUE);
        let page = Self::page(model, controller);
        ModListView::show(&mut ui.new_child(UiBuilder::new().max_rect(Self::sidebar(screen))), model, page, icons, out);
        let mut pane_ui = ui.new_child(UiBuilder::new().max_rect(Self::pane(screen)));
        match page {
            PanelPage::Mod(index) => ModDetailView::show(&mut pane_ui, model, controller, icons, index, out),
            PanelPage::Conflicts => ConflictsView::show(&mut pane_ui, model, out),
            PanelPage::Keybinds => KeybindsPage::show(&mut pane_ui, model, controller, out),
            PanelPage::ManagerSettings => ManagerSettingsView::show(&mut pane_ui, model, out),
        }
    }
}
