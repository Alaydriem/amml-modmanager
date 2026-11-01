use amm_core::structs::registry::ModIndex;
use amm_ui::id::AmmId;
use amm_ui::model::PanelTab;
use amml_ui::id::AmmlUiId;
use egui::Id;

/// An id enum from another crate, with a derived `Hash`.
#[derive(Debug, Clone, Copy, Hash)]
enum OtherId {
    First,
    Second,
    Third,
}

#[test]
fn an_amm_id_never_equals_the_same_numbered_variant_of_another_enum() {
    assert_ne!(Id::unique(AmmId::ModList), Id::unique(OtherId::First));
    assert_ne!(Id::unique(AmmId::Pane(ModIndex(0), PanelTab::General)), Id::unique(OtherId::Second));
    assert_ne!(Id::unique(AmmId::Tabs), Id::unique(OtherId::Third));
    assert_ne!(Id::unique(AmmId::ModList), Id::unique(AmmlUiId::Locale));
    assert_ne!(Id::unique(AmmId::Tabs), Id::unique(AmmlUiId::UnderlineLeft));
}

#[test]
fn amm_id_variants_keep_their_own_ids() {
    assert_ne!(Id::unique(AmmId::Resolution(0)), Id::unique(AmmId::Resolution(1)));
    assert_ne!(Id::unique(AmmId::Resolution(0)), Id::unique(AmmId::Setting(0)));
    assert_ne!(
        Id::unique(AmmId::Pane(ModIndex(0), PanelTab::General)),
        Id::unique(AmmId::Pane(ModIndex(0), PanelTab::Keybinds))
    );
}
