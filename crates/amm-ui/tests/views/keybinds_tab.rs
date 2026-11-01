use amm_ui::model::{PanelTab, RebindTarget};

use crate::support::{FourMods, MINIMAP, PanelHarness};

// the four mods with two of freecam's binds clashing: path markers with Light Level's toggle
// (both Ctrl+B), and cycle mode with the minimap's world map (both M)
fn two_clashes() -> FourMods {
    let mut mods = FourMods::new(MINIMAP);
    let (levels, freecam) = (mods.levels, mods.freecam);
    for (index, path, chord) in [(levels, "keys.toggle", "\"Ctrl+B\""), (freecam, "keys.cycle_mode", "\"M\"")] {
        mods.manager.set_setting(index, path, chord, 0).unwrap_or_else(|error| panic!("{path} rebinds: {error}"));
    }
    mods
}

#[test]
fn click_row_starts_capture() {
    let mods = FourMods::new(MINIMAP);
    let minimap = mods.minimap;
    let mut harness = PanelHarness::new(mods, "");
    harness.open(minimap, PanelTab::Keybinds);
    let row = harness.text_rect("Toggle world map").unwrap_or_else(|| panic!("the keybind is painted"));

    harness.click(row.center());

    assert_eq!(harness.controller.rebinding(), Some(RebindTarget::Mod { mod_index: minimap, action: 0 }));
    assert!(harness.text_rect("Press a key").is_some(), "the row shows the capture prompt");
}

#[test]
fn conflict_row_shows_the_other_mod() {
    let mods = two_clashes();
    let freecam = mods.freecam;
    let mut harness = PanelHarness::new(mods, "");
    harness.open(freecam, PanelTab::Keybinds);

    assert!(harness.text_rect("Also bound by Light Level").is_some(), "no reason in {:?}", harness.texts());
}

#[test]
fn banner_counts_conflicts() {
    let mods = two_clashes();
    let freecam = mods.freecam;
    let mut harness = PanelHarness::new(mods, "");
    harness.open(freecam, PanelTab::Keybinds);

    let title =
        harness.text_rect("2 key conflicts").unwrap_or_else(|| panic!("no banner title in {:?}", harness.texts()));
    let tail = harness
        .texts()
        .into_iter()
        .find(|(text, rect)| text.starts_with("is also bound by") && title.y_range().contains(rect.center().y))
        .unwrap_or_else(|| panic!("no banner tail on the title's line"));
    let caps: Vec<_> = harness
        .texts()
        .into_iter()
        .filter(|(_, rect)| rect.left() > title.right() && rect.right() < tail.1.left())
        .filter(|(_, rect)| title.y_range().contains(rect.center().y))
        .collect();

    assert!(!caps.is_empty(), "the chord's caps sit between the title and the tail");
}
