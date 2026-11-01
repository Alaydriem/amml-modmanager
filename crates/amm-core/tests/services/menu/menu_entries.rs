use amm_core::errors::ManagerError;
use amm_core::services::menu::MenuEntries;
use amm_core::structs::frame::MenuScreen;
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::registry::ModIndex;

const DEJAVU: ModIndex = ModIndex(0);
const MINIMAP: ModIndex = ModIndex(1);

#[test]
fn ids_count_from_one_for_each_mod() {
    let mut entries = MenuEntries::new();

    let first = entries.add(DEJAVU, "Dejavu", "DV", MenuScreens::START);
    let second = entries.add(DEJAVU, "Capture", "CP", MenuScreens::PAUSE);
    let other = entries.add(MINIMAP, "Map", "MM", MenuScreens::START);

    assert_eq!(first.ok(), Some(MenuEntryId(1)));
    assert_eq!(second.ok(), Some(MenuEntryId(2)));
    assert_eq!(other.ok(), Some(MenuEntryId(1)));
}

#[test]
fn entries_for_pause_hide_start_only_entries() {
    let mut entries = MenuEntries::new();
    let _ = entries.add(DEJAVU, "Dejavu", "DV", MenuScreens::START);
    let _ = entries.add(MINIMAP, "Map", "MM", MenuScreens::BOTH);

    let start: Vec<&str> = entries.for_screen(MenuScreen::Start).map(|entry| entry.label.as_str()).collect();
    let pause: Vec<&str> = entries.for_screen(MenuScreen::Pause).map(|entry| entry.label.as_str()).collect();
    let none = entries.for_screen(MenuScreen::None).count();

    assert_eq!(start, ["Dejavu", "Map"]);
    assert_eq!(pause, ["Map"]);
    assert_eq!(none, 0);
}

#[test]
fn forget_drops_only_that_mods_entries() {
    let mut entries = MenuEntries::new();
    let _ = entries.add(DEJAVU, "Dejavu", "DV", MenuScreens::START);
    let _ = entries.add(MINIMAP, "Map", "MM", MenuScreens::START);

    entries.forget(DEJAVU);

    let left: Vec<&str> = entries.for_screen(MenuScreen::Start).map(|entry| entry.label.as_str()).collect();
    assert_eq!(left, ["Map"]);
    assert!(!entries.contains(DEJAVU, MenuEntryId(1)));
    assert!(entries.contains(MINIMAP, MenuEntryId(1)));
}

#[test]
fn fifth_entry_for_one_mod_is_refused() {
    let mut entries = MenuEntries::new();
    for _ in 0..MenuEntries::PER_MOD {
        assert!(entries.add(DEJAVU, "Entry", "E", MenuScreens::START).is_ok());
    }

    let fifth = entries.add(DEJAVU, "Entry", "E", MenuScreens::START);

    assert!(matches!(fifth, Err(ManagerError::MenuEntryLimit)));
}

#[test]
fn ninth_entry_in_total_is_refused() {
    let mut entries = MenuEntries::new();
    for slot in 0..2u8 {
        for _ in 0..MenuEntries::PER_MOD {
            assert!(entries.add(ModIndex(slot), "Entry", "E", MenuScreens::START).is_ok());
        }
    }

    let ninth = entries.add(ModIndex(2), "Entry", "E", MenuScreens::START);

    assert_eq!(MenuEntries::TOTAL, 8);
    assert!(matches!(ninth, Err(ManagerError::MenuEntryLimit)));
}

#[test]
fn long_label_long_mark_and_no_screen_are_refused() {
    let mut entries = MenuEntries::new();

    let label = entries.add(DEJAVU, &"x".repeat(MenuEntries::LABEL_MAX + 1), "DV", MenuScreens::START);
    let mark = entries.add(DEJAVU, "Dejavu", "DEJA", MenuScreens::START);
    let empty = entries.add(DEJAVU, "", "DV", MenuScreens::START);
    let screens = entries.add(DEJAVU, "Dejavu", "DV", MenuScreens::from_bits(0));

    for refused in [label, mark, empty, screens] {
        assert!(matches!(refused, Err(ManagerError::MenuEntry)));
    }
}
