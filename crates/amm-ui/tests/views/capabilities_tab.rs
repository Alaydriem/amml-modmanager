use amm_ui::model::PanelTab;

use crate::support::{FourMods, MINIMAP, PanelHarness};

#[test]
fn undeclared_capability_is_muted() {
    let mods = FourMods::new(MINIMAP);
    let minimap = mods.minimap;
    let mut harness = PanelHarness::new(mods, "");
    harness.open(minimap, PanelTab::Capabilities);

    // the minimap claims Input, HUD and Overlay, not world draw
    assert!(harness.text_rect("Not declared").is_some(), "no undeclared row in {:?}", harness.texts());
    assert_eq!(harness.text_faded("World draw"), Some(true), "the world draw row is not muted");
    assert_eq!(harness.text_faded("HUD"), Some(false), "a declared row is muted");
}
