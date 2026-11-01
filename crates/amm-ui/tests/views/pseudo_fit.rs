use amm_core::structs::descriptor::Capability;
use amm_ui::model::{PanelPage, PanelTab};
use amm_ui::views::{HeaderView, PanelLayout};
use egui::Rect;

use crate::support::{FourMods, MINIMAP, PanelHarness};

// Review Focus 4: under en_XA every text is cut inside its part. A cut text keeps its source text,
// so these tests check the painted rects.

// the four mods with a key clash, a suspension and a denied toast, drawn in en_XA
fn pseudo() -> PanelHarness {
    let mut mods = FourMods::new(MINIMAP);
    let (levels, dejavu, freecam) = (mods.levels, mods.dejavu, mods.freecam);
    mods.manager.set_setting(levels, "keys.toggle", "\"Ctrl+B\"", 0).unwrap_or_else(|error| panic!("{error}"));
    mods.manager.acquire(dejavu, Capability::Camera, 0).unwrap_or_else(|error| panic!("{error}"));
    mods.manager.acquire(freecam, Capability::Camera, 0).unwrap_or_else(|error| panic!("{error}"));
    PanelHarness::on_screen(mods, "en_XA", PanelHarness::SMALL_SCREEN)
}

// the texts that start inside `part` and end past its right edge
fn outside(harness: &PanelHarness, part: Rect) -> Vec<(String, Rect)> {
    harness
        .texts()
        .into_iter()
        .filter(|(_, rect)| part.contains(rect.left_top()))
        .filter(|(_, rect)| rect.right() > part.right() + 0.5)
        .collect()
}

#[test]
fn pseudo_header_stays_in_the_panel() {
    let harness = pseudo();
    let panel = PanelLayout::panel_rect(harness.screen);
    let header = Rect::from_min_size(panel.min, egui::vec2(panel.width(), HeaderView::height()));

    let outside = outside(&harness, header);
    assert!(outside.is_empty(), "header texts past the panel's edge: {outside:?}");
}

#[test]
fn pseudo_pane_pages_stay_in_the_pane() {
    let mut harness = pseudo();
    let pane = PanelLayout::pane(harness.screen);
    let freecam = harness.mods.freecam;
    for tab in [PanelTab::General, PanelTab::Keybinds, PanelTab::Capabilities] {
        harness.open(freecam, tab);
        let outside = outside(&harness, pane);
        assert!(outside.is_empty(), "{tab:?} texts past the pane's edge: {outside:?}");
    }
    for page in [PanelPage::Conflicts, PanelPage::Keybinds, PanelPage::ManagerSettings] {
        harness.show_page(page);
        let outside = outside(&harness, pane);
        assert!(outside.is_empty(), "{page:?} texts past the pane's edge: {outside:?}");
    }
}

#[test]
fn pseudo_toasts_stay_in_their_cards() {
    let harness = pseudo();
    let screen = harness.screen;
    let (right, bottom) = (screen.right() - 24.0, screen.bottom() - 24.0);
    let column = Rect::from_min_max(egui::pos2(right - 480.0, screen.top()), egui::pos2(right, bottom));

    let outside = outside(&harness, column);
    assert!(outside.is_empty(), "toast texts past the card's edge: {outside:?}");
}
