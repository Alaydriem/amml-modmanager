use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::errors::ManagerError;
use amm_core::services::manager::ManagerService;
use amm_core::structs::icon::ModIcon;
use amm_core::structs::registry::ModIndex;

const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");
const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");

struct IconCase;

impl IconCase {
    fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> ModIndex {
        manager.register(hcl, None, &Path::new("mods").join(id)).unwrap_or_else(|error| panic!("{id}: {error}")).0
    }

    fn icon() -> ModIcon {
        ModIcon { width: 2, height: 2, rgba: vec![255; 16] }
    }
}

#[test]
fn unknown_slot_is_unknown_mod() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);

    let result = manager.set_icon(ModIndex(3), Some(IconCase::icon()));

    assert!(matches!(result, Err(ManagerError::UnknownMod)));
    assert!(manager.icon(ModIndex(3)).is_none());
}

#[test]
fn unregister_frees_the_icon() {
    let mut manager = ManagerService::new(ManagerConfig::default(), None);
    let first = IconCase::register(&mut manager, DEJAVU, "dejavu");
    assert!(manager.set_icon(first, Some(IconCase::icon())).is_ok());
    assert!(manager.icon(first).is_some());

    manager.unregister(first);
    let second = IconCase::register(&mut manager, MINIMAP, "minimap");

    assert_eq!(second, first);
    assert!(manager.icon(second).is_none());
}
