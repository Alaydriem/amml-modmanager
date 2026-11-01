use amm_ui::model::{PanelAction, PanelPage};
use amm_ui::views::PanelLayout;

use crate::support::{FourMods, MINIMAP, PanelHarness};

fn settings(mods: FourMods) -> PanelHarness {
    let mut harness = PanelHarness::new(mods, "");
    harness.show_page(PanelPage::ManagerSettings);
    harness
}

#[test]
fn picking_a_language_sends_an_action() {
    let mut harness = settings(FourMods::new(MINIMAP));
    let field = harness.text_rect("English").unwrap_or_else(|| panic!("the language field is painted"));

    harness.click(field.center());
    let option = harness.text_rect("en_XA").unwrap_or_else(|| panic!("no en_XA option in {:?}", harness.texts()));
    harness.click(option.center());

    assert!(harness.actions.contains(&PanelAction::SetLanguage("en_XA".to_owned())), "{:?}", harness.actions);
}

#[test]
fn unknown_code_selects_english() {
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_locale("xx");

    let harness = settings(mods);

    assert!(harness.text_rect("English").is_some(), "the field does not show English: {:?}", harness.texts());
}

#[test]
fn a_code_with_spaces_selects_its_language() {
    // `LocaleSetting::from_code` trims the code, so the panel draws en_XA; the field must say so too
    let mut mods = FourMods::new(MINIMAP);
    mods.manager.set_locale(" en_XA ");

    let harness = settings(mods);

    assert!(harness.text_rect("en_XA").is_some(), "the field does not show en_XA: {:?}", harness.texts());
}

#[test]
fn settings_page_has_only_panel_and_language_cards() {
    let harness = settings(FourMods::new(MINIMAP));

    assert_eq!(harness.count("PANEL"), 1, "{:?}", harness.texts());
    assert_eq!(harness.count("LANGUAGE"), 1, "{:?}", harness.texts());
    assert_eq!(harness.count("MODS"), 0, "{:?}", harness.texts());
    assert_eq!(harness.count("LIMITS"), 0, "{:?}", harness.texts());
    let note = "Limits are read when the game starts. Change them in the manager's config.hcl.";
    assert_eq!(harness.count(note), 0, "{:?}", harness.texts());
    let sidebar = PanelLayout::sidebar(PanelHarness::SCREEN);
    let freecam = harness.text_rect("Freecam").unwrap_or_else(|| panic!("the Freecam row is painted"));
    assert_eq!(harness.count("Freecam"), 1, "{:?}", harness.texts());
    assert!(sidebar.contains_rect(freecam), "{freecam:?} is outside {sidebar:?}");
}
