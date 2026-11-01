use amm_ui::model::PanelAction;
use amm_ui::views::PanelLayout;
use amml_ui::tokens::{SpacingToken, spacing};
use amml_ui::widget::Toggle;
use egui::pos2;

use crate::support::{FourMods, IconFixture, MINIMAP, PanelHarness};

#[test]
fn mod_row_click_selects_and_toggle_enables() {
    let mods = FourMods::new(MINIMAP);
    let freecam = mods.freecam;
    let mut harness = PanelHarness::new(mods, "");
    let name = harness.text_rect("Freecam").unwrap_or_else(|| panic!("the Freecam row is painted"));

    harness.click(name.center());
    // the avatar letter is centred on the row, so its centre is the row's middle line
    let letter = harness.text_rect("F").unwrap_or_else(|| panic!("the Freecam avatar is painted"));
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);
    let pad = <spacing::M as SpacingToken>::VALUE;
    harness.click(pos2(sidebar.right() - pad - Toggle::TRACK.x / 2.0, letter.center().y));

    assert_eq!(harness.actions, [PanelAction::Select(Some(freecam)), PanelAction::SetEnabled(freecam, false)]);
}

#[test]
fn conflict_entry_shows_the_count() {
    let mut mods = FourMods::new(MINIMAP);
    // Light Level's toggle on Minimap's Ctrl+B: a key clash
    mods.manager
        .set_setting(mods.levels, "keys.toggle", "\"Ctrl+B\"", 0)
        .unwrap_or_else(|error| panic!("the toggle rebinds: {error}"));
    let count = mods.manager.clash_report().conflict_count();
    let harness = PanelHarness::new(mods, "");

    let entry = harness.text_rect("Conflicts").unwrap_or_else(|| panic!("the Conflicts entry is painted"));
    let on_its_line = |(text, rect): &(String, egui::Rect)| {
        *text == count.to_string() && rect.left() > entry.right() && entry.y_range().contains(rect.center().y)
    };

    assert!(count >= 1, "the four mods clash");
    assert!(harness.texts().iter().any(on_its_line), "a chip with {count} is on the Conflicts entry's line");
    assert!(!harness.texts().iter().any(|(text, _)| text.contains('(')), "the count is not in the label");
}

#[test]
fn pseudo_mod_list_stays_in_the_sidebar() {
    let long = "Minimap with a very long display name that runs past the sid";
    let mods = FourMods::new(&MINIMAP.replace("\"Minimap\"", &format!("\"{long}\"")));
    let harness = PanelHarness::new(mods, "en_XA");
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);

    let outside: Vec<_> = harness
        .texts()
        .into_iter()
        .filter(|(_, rect)| sidebar.x_range().contains(rect.left()) && rect.top() >= sidebar.top())
        .filter(|(_, rect)| rect.right() > sidebar.right() + 0.5)
        .collect();

    assert_eq!(long.chars().count(), 60);
    assert!(outside.is_empty(), "these texts pass the sidebar's right edge {}: {outside:?}", sidebar.right());
}

#[test]
fn mod_rows_show_no_capability_chips() {
    let harness = PanelHarness::new(FourMods::new(MINIMAP), "");
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);

    let chips: Vec<_> = harness
        .texts()
        .into_iter()
        .filter(|(text, rect)| {
            sidebar.contains(rect.center()) && ["Camera", "Input", "HUD", "Overlay", "World"].contains(&text.as_str())
        })
        .collect();

    assert!(chips.is_empty(), "the sidebar paints these chips: {chips:?}");
}

#[test]
fn manager_group_order_is_settings_keybinds_conflicts() {
    let harness = PanelHarness::new(FourMods::new(MINIMAP), "");
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);
    let top = |label: &str| {
        harness
            .texts()
            .into_iter()
            .filter(|(text, rect)| text == label && sidebar.contains(rect.center()))
            .map(|(_, rect)| rect.top())
            .next()
            .unwrap_or_else(|| panic!("{label} is painted in the sidebar"))
    };

    let (settings, keybinds, conflicts) = (top("Manager settings"), top("Keybinds"), top("Conflicts"));

    assert!(settings < keybinds && keybinds < conflicts, "order tops: {settings} {keybinds} {conflicts}");
}

#[test]
fn mod_with_icon_draws_image_and_no_letter() {
    let plain = PanelHarness::new(FourMods::new(MINIMAP), "");
    let mut mods = FourMods::new(MINIMAP);
    mods.manager
        .set_icon(mods.freecam, Some(IconFixture::solid(8, 8, [255, 0, 0, 255])))
        .expect("freecam is registered");
    let harness = PanelHarness::new(mods, "");
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);

    let in_sidebar =
        |harness: &PanelHarness| harness.image_rects().into_iter().filter(|rect| sidebar.contains_rect(*rect)).count();

    assert_eq!(in_sidebar(&plain), 0);
    assert_eq!(in_sidebar(&harness), 1);
    assert_eq!(harness.count("F"), plain.count("F") - 1);
}

#[test]
fn mod_without_icon_draws_letter() {
    let harness = PanelHarness::new(FourMods::new(MINIMAP), "");

    assert!(harness.count("F") >= 1);
}
