use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::services::manager::ManagerService;
use amm_core::structs::event::ModEvent;
use amm_core::structs::registry::ModIndex;

const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");
const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");

fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> ModIndex {
    manager.register(hcl, None, &Path::new("mods").join(id)).unwrap_or_else(|error| panic!("{id}: {error}")).0
}

fn events(manager: &mut ManagerService, index: ModIndex) -> Vec<ModEvent> {
    let mut out = [ModEvent::Enabled; 64];
    let count = manager.poll_events(index, &mut out);
    out[..count].to_vec()
}

#[test]
fn older_mod_gets_no_locale_event() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let reader = register(&mut manager, DEJAVU, "dejavu");
    let older = register(&mut manager, MINIMAP, "minimap");
    assert_eq!(manager.read_locale(reader), "");
    let _ = events(&mut manager, reader);
    let _ = events(&mut manager, older);

    manager.set_locale("en_XA");

    assert_eq!(manager.locale(), "en_XA");
    assert_eq!(events(&mut manager, reader), [ModEvent::LocaleChanged]);
    assert!(events(&mut manager, older).is_empty(), "a mod that never read the locale got the event");
}

#[test]
fn set_locale_to_the_same_code_sends_nothing() {
    let config = ManagerConfig { language: "en_XA".to_owned(), ..ManagerConfig::default() };
    let mut manager = ManagerService::new(config, None);
    let reader = register(&mut manager, DEJAVU, "dejavu");
    assert_eq!(manager.read_locale(reader), "en_XA");
    let _ = events(&mut manager, reader);

    manager.set_locale("en_XA");

    assert!(events(&mut manager, reader).is_empty());
}

#[test]
fn a_mod_that_unregisters_stops_being_a_reader() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let first = register(&mut manager, DEJAVU, "dejavu");
    let _ = manager.read_locale(first);
    manager.unregister(first);
    // the next mod may take the same slot; it has not read the locale
    let second = register(&mut manager, MINIMAP, "minimap");
    let _ = events(&mut manager, second);

    manager.set_locale("en_XA");

    assert!(events(&mut manager, second).is_empty());
}
