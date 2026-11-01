use amm_core::structs::descriptor::{ApplyMode, FieldKind, SettingField};
use amm_core::structs::registry::ModIndex;
use amm_ui::model::PanelModel;
use amm_ui::views::ValueText;
use amml_i18n::localize::Localizer;
use amml_ui::widget::DragUnit;

use crate::support::{DEMO, FourMods, MINIMAP_SETTINGS};

fn field(mods: &FourMods, index: ModIndex, path: &str) -> SettingField {
    let model = PanelModel::new(&mods.manager, "1.4.1", 0);
    let registered = model.registered(index).unwrap_or_else(|| panic!("the mod is registered"));
    let field = registered.descriptor.settings.iter().flat_map(|section| &section.fields).find(|f| f.path == path);
    field.cloned().unwrap_or_else(|| panic!("no field {path}"))
}

#[test]
fn a_float_shows_the_decimals_its_step_implies() {
    let mods = FourMods::new(DEMO);
    let opacity = field(&mods, mods.minimap, "card.opacity");

    assert_eq!(ValueText::display(&opacity, &hcl::Value::from(0.0)), "0.00");
    assert_eq!(ValueText::display(&opacity, &hcl::Value::from(0.9)), "0.90");
}

#[test]
fn a_float_with_a_unit_and_a_quarter_step_shows_two_decimals_and_the_unit() {
    let mods = FourMods::new(MINIMAP_SETTINGS);
    let zoom = field(&mods, mods.minimap, "map.zoom");

    assert_eq!(ValueText::display(&zoom, &hcl::Value::from(2.0)), "2.00x");
}

#[test]
fn an_int_shows_a_whole_number() {
    let mods = FourMods::new(DEMO);
    let rows = field(&mods, mods.minimap, "card.rows");

    assert_eq!(ValueText::display(&rows, &hcl::Value::from(8)), "8");
}

fn float_field(min: f64, max: f64, step: f64, unit: Option<&str>) -> SettingField {
    SettingField {
        path: "motion.max_speed".to_string(),
        label: "Max speed".to_string(),
        help: String::new(),
        kind: FieldKind::Float { min, max, step, unit: unit.map(str::to_string) },
        default: hcl::Value::from(min),
        apply: ApplyMode::Live,
    }
}

#[test]
fn a_word_unit_is_set_apart_from_the_number_by_a_space() {
    let speed = float_field(1.0, 400.0, 0.5, Some("blocks/s"));

    assert_eq!(ValueText::display(&speed, &hcl::Value::from(400.0)), "400.0 blocks/s");
}

#[test]
fn a_one_character_unit_joins_and_a_word_unit_is_spaced() {
    assert!(matches!(ValueText::unit(Some("x")), DragUnit::Joined(_)));
    assert!(matches!(ValueText::unit(Some("%")), DragUnit::Joined(_)));
    assert!(matches!(ValueText::unit(Some("blocks/s")), DragUnit::Text(_)));
    assert!(matches!(ValueText::unit(None), DragUnit::None));
}

#[test]
fn an_on_off_change_names_the_value_in_words() {
    let mods = FourMods::new(DEMO);
    let rows = field(&mods, mods.minimap, "card.rows");
    let toggle = SettingField { kind: FieldKind::Bool, ..rows };
    let english = Localizer::english();

    let note = ValueText::changed_in_game(&toggle, &hcl::Value::from(true));

    assert_eq!(note.resolve(&english), "Changed in game to on");
}
