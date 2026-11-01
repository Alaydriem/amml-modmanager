use amml_ui::tokens::{ColorToken, color};
use amml_ui::widget::MetaSeparator;

use crate::support::{FourMods, MINIMAP, PanelHarness};

#[test]
fn header_meta_is_three_parts() {
    let harness = PanelHarness::new(FourMods::new(MINIMAP), "");

    let installed = harness.text_rect("4 mods installed").unwrap_or_else(|| panic!("the mod count is painted"));
    let line = installed.y_range();
    let parts = ["4 mods installed", "4 enabled", "v1.4.1"];
    let dots: Vec<_> = harness
        .filled_rects(MetaSeparator::SIDE, <color::TextFaint as ColorToken>::VALUE)
        .into_iter()
        .filter(|dot| line.contains(dot.center().y))
        .collect();

    for part in parts {
        assert!(harness.text_rect(part).is_some(), "{part} is painted on its own");
    }
    assert_eq!(dots.len(), 2, "two drawn dots join the three parts");
    let header_texts: Vec<_> = harness.texts().into_iter().filter(|(_, rect)| line.contains(rect.center().y)).collect();
    assert!(
        !header_texts.iter().any(|(text, _)| text.contains('\u{b7}')),
        "a header text holds a dot: {header_texts:?}"
    );
}
