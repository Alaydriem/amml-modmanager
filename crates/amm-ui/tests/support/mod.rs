mod icon_fixture;
mod panel_harness;

pub use icon_fixture::IconFixture;
pub use panel_harness::PanelHarness;

use std::path::Path;

use amm_core::config::ManagerConfig;
use amm_core::services::manager::ManagerService;
use amm_core::structs::registry::ModIndex;

pub const MINIMAP: &str = include_str!("../../../amm-core/fixtures/four_mods/minimap.hcl");
pub const FREECAM: &str = include_str!("../../../amm-core/fixtures/four_mods/freecam.hcl");
pub const LEVELS: &str = include_str!("../../../amm-core/fixtures/four_mods/levels.hcl");
pub const DEJAVU: &str = include_str!("../../../amm-core/fixtures/four_mods/dejavu.hcl");
pub const DEMO: &str = include_str!("demo_descriptor.hcl");
pub const MINIMAP_SETTINGS: &str = include_str!("../../fixtures/minimap_settings.hcl");

// The four fixture mods in registration order: minimap, freecam, levels, dejavu. The first slot takes the
// descriptor given, which need not be the minimap.
pub struct FourMods {
    pub manager: ManagerService,
    pub minimap: ModIndex,
    pub freecam: ModIndex,
    pub levels: ModIndex,
    pub dejavu: ModIndex,
}

impl FourMods {
    pub fn new(minimap: &str) -> Self {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let minimap = Self::register(&mut manager, minimap, "minimap");
        let freecam = Self::register(&mut manager, FREECAM, "freecam");
        let levels = Self::register(&mut manager, LEVELS, "levels");
        let dejavu = Self::register(&mut manager, DEJAVU, "dejavu");
        Self { manager, minimap, freecam, levels, dejavu }
    }

    fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> ModIndex {
        let dir = std::env::temp_dir().join("amm-ui-tests").join(id);
        manager.register(hcl, None, Path::new(&dir)).unwrap_or_else(|error| panic!("{id} does not register: {error}")).0
    }
}
