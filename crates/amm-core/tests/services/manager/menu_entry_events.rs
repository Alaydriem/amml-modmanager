use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::services::manager::ManagerService;
use amm_core::structs::event::ModEvent;
use amm_core::structs::frame::MenuScreen;
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::registry::ModIndex;

const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");
const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");

fn manager() -> ManagerService {
    ManagerService::new(ManagerConfig::default(), None)
}

fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> ModIndex {
    manager.register(hcl, None, &Path::new("mods").join(id)).unwrap_or_else(|error| panic!("{id}: {error}")).0
}

fn events(manager: &mut ManagerService, index: ModIndex) -> Vec<ModEvent> {
    let mut out = [ModEvent::Enabled; 64];
    let count = manager.poll_events(index, &mut out);
    out[..count].to_vec()
}

#[test]
fn click_on_entry_sends_menu_entry_clicked_to_owner_only() {
    let mut manager = manager();
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    let minimap = register(&mut manager, MINIMAP, "minimap");
    let id = manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START);
    let _ = events(&mut manager, dejavu);
    let _ = events(&mut manager, minimap);

    manager.menu_entry_clicked(dejavu, MenuEntryId(1));

    assert_eq!(id.ok(), Some(MenuEntryId(1)));
    assert_eq!(events(&mut manager, dejavu), [ModEvent::MenuEntryClicked { entry: 1 }]);
    assert!(events(&mut manager, minimap).is_empty());
}

#[test]
fn click_on_a_forgotten_entry_sends_nothing() {
    let mut manager = manager();
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    let _ = events(&mut manager, dejavu);

    manager.menu_entry_clicked(dejavu, MenuEntryId(3));

    assert!(events(&mut manager, dejavu).is_empty());
}

#[test]
fn unregister_drops_the_mods_entries() {
    let mut manager = manager();
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    let _ = manager.register_menu_entry(dejavu, "Dejavu", "DV", MenuScreens::START);

    manager.unregister(dejavu);

    assert_eq!(manager.menu_entries().for_screen(MenuScreen::Start).count(), 0);
}

#[test]
fn menu_entry_clicked_survives_a_full_queue() {
    assert!(ModEvent::MenuEntryClicked { entry: 1 }.is_protected());
}
