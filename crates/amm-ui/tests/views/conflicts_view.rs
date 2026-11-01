use amm_ui::model::PanelPage;

use crate::support::{FourMods, MINIMAP, PanelHarness};

#[test]
fn resolution_dropdowns_are_apart() {
    // the minimap shares the camera too: two suspensions under DejaVu's exclusive camera
    let mods = FourMods::new(&MINIMAP.replace("claim \"Input\"", "claim \"Camera\""));
    let mut harness = PanelHarness::new(mods, "");
    harness.show_page(PanelPage::Conflicts);
    let fields: Vec<_> = harness.texts().into_iter().filter(|(text, _)| text.starts_with("Suspend ")).collect();
    assert_eq!(fields.len(), 2, "two resolution fields: {:?}", harness.texts());
    assert_eq!(harness.count("Coexist"), 0, "a list is open before any click");

    harness.click(fields[0].1.center());

    assert_eq!(harness.count("Coexist"), 1, "one list opens, not both");
}
