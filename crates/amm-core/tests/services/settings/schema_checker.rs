use amm_core::services::settings::{ColorCodec, SchemaChecker};
use amm_core::structs::descriptor::{ApplyMode, ColorEncoding, EnumOption, FieldKind, SettingField, SettingsSection};
use amm_core::structs::settings::{ConfigWarning, HclDocument};

fn field(path: &str, kind: FieldKind, default: hcl::Value) -> SettingField {
    SettingField { path: path.into(), label: "L".into(), help: String::new(), kind, default, apply: ApplyMode::Live }
}

fn section(fields: Vec<SettingField>) -> Vec<SettingsSection> {
    vec![SettingsSection { id: "s".into(), title: "S".into(), fields }]
}

fn zoom() -> SettingField {
    field("mini_map.zoom", FieldKind::Int { min: 1, max: 8, step: 1 }, hcl::Value::from(2))
}

fn scale() -> SettingField {
    field("mini_map.scale", FieldKind::Float { min: 0.5, max: 2.0, step: 0.1, unit: None }, hcl::Value::from(1.0))
}

fn color(encoding: ColorEncoding, default: hcl::Value) -> SettingField {
    field("card.color", FieldKind::Color { alpha: true, encoding }, default)
}

#[test]
fn int_out_of_range_is_clamped_with_a_warning() {
    let document = HclDocument::new("mini_map {\n  zoom = 40\n  scale = 0.1\n}\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![zoom(), scale()]), document);

    assert!(checked.changed());
    assert_eq!(
        checked.warnings,
        vec![
            ConfigWarning::Clamped { path: "mini_map.zoom".into() },
            ConfigWarning::Clamped { path: "mini_map.scale".into() },
        ]
    );
    assert_eq!(checked.document.get("mini_map.zoom"), Some(hcl::Value::from(8)));
    assert_eq!(checked.document.get("mini_map.scale"), Some(hcl::Value::from(0.5)));
    assert_eq!(
        SchemaChecker::check_value(&zoom(), &hcl::Value::from(-3)),
        Err(ConfigWarning::Clamped { path: "mini_map.zoom".into() })
    );
}

#[test]
fn wrong_type_takes_the_default() {
    let document = HclDocument::new("mini_map {\n  zoom = \"big\"\n  scale = 1.5\n}\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![zoom(), scale()]), document);

    assert_eq!(checked.warnings, vec![ConfigWarning::Defaulted { path: "mini_map.zoom".into() }]);
    assert_eq!(checked.document.get("mini_map.zoom"), Some(hcl::Value::from(2)));
    assert_eq!(checked.document.get("mini_map.scale"), Some(hcl::Value::from(1.5)));
    assert_eq!(
        SchemaChecker::check_value(&zoom(), &hcl::Value::from(2.5)),
        Err(ConfigWarning::Defaulted { path: "mini_map.zoom".into() })
    );
}

#[test]
fn missing_block_takes_defaults() {
    let document = HclDocument::new("# only a comment\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![zoom(), scale()]), document);

    assert!(checked.changed());
    assert_eq!(checked.document.get("mini_map.zoom"), Some(hcl::Value::from(2)));
    assert_eq!(checked.document.get("mini_map.scale"), Some(hcl::Value::from(1.0)));
    assert_eq!(checked.document.text(), "# only a comment\nmini_map {\n  zoom = 2\n  scale = 1\n}\n");
}

#[test]
fn a_valid_file_is_not_changed() {
    let text = "# kept\nmini_map {\n  zoom = 3 # mine\n  scale = 1.25\n}\n";

    let checked = SchemaChecker::check(&section(vec![zoom(), scale()]), HclDocument::new(text).unwrap());

    assert!(!checked.changed());
    assert_eq!(checked.document.text(), text);
}

#[test]
fn comments_and_unknown_attributes_survive_a_check_and_a_write() {
    let text = "# note\nmini_map {\n  zoom = 99\n  extra = 1\n}\n\nwaypoint \"home\" { }\n";

    let checked = SchemaChecker::check(&section(vec![zoom()]), HclDocument::new(text).unwrap());
    let written = checked.document.with("mini_map.zoom", &hcl::Value::from(4)).text();

    assert!(written.contains("# note"), "{written}");
    assert!(written.contains("extra = 1"), "{written}");
    assert!(written.contains("waypoint \"home\""), "{written}");
    assert!(written.contains("zoom = 4"), "{written}");
}

#[test]
fn enum_value_not_in_options_takes_the_default() {
    let options = vec![
        EnumOption { value: hcl::Value::from("flat"), label: "Flat".into() },
        EnumOption { value: hcl::Value::from("gradient"), label: "Gradient".into() },
    ];
    let style = field("style", FieldKind::Enum { options }, hcl::Value::from("flat"));
    let document = HclDocument::new("style = \"neon\"\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![style.clone()]), document);

    assert_eq!(checked.warnings, vec![ConfigWarning::Defaulted { path: "style".into() }]);
    assert_eq!(checked.document.get("style"), Some(hcl::Value::from("flat")));
    assert_eq!(SchemaChecker::check_value(&style, &hcl::Value::from("gradient")), Ok(hcl::Value::from("gradient")));
}

const FF4040FF: [u8; 4] = [0xFF, 0x40, 0x40, 0xFF];

#[test]
fn valid_color_in_each_encoding_is_unchanged() {
    let cases = [
        (ColorEncoding::FloatArray4, "[1.0, 0.25, 0.25, 1.0]", hcl::Value::from(vec![1.0, 0.25, 0.25, 1.0]), FF4040FF),
        (ColorEncoding::ByteArray4, "[255, 64, 64, 255]", hcl::Value::from(vec![255, 64, 64, 255]), FF4040FF),
        // 4282400767 is 0xFF403FFF
        (ColorEncoding::PackedRgbaU32, "4282400767", hcl::Value::from(4282400767u64), [0xFF, 0x40, 0x3F, 0xFF]),
        (ColorEncoding::HexString, "\"#FF4040FF\"", hcl::Value::from("#FF4040FF"), FF4040FF),
    ];
    for (encoding, literal, value, rgba) in cases {
        let text = format!("card {{\n  color = {literal}\n}}\n");
        let default = ColorCodec::from_rgba([0, 0, 0, 255], encoding);

        let checked = SchemaChecker::check(&section(vec![color(encoding, default)]), HclDocument::new(&text).unwrap());

        assert!(!checked.changed(), "{encoding:?}: {:?}", checked.warnings);
        assert_eq!(checked.document.text(), text);
        assert_eq!(checked.document.get("card.color"), Some(value.clone()));
        assert_eq!(ColorCodec::to_rgba(&value, encoding), Some(rgba), "{encoding:?}");
    }
}

#[test]
fn colors_encode_to_each_format_and_decode_back() {
    let rgba = FF4040FF;

    assert_eq!(ColorCodec::from_rgba(rgba, ColorEncoding::PackedRgbaU32), hcl::Value::from(0xFF4040FFu32));
    assert_eq!(ColorCodec::from_rgba(rgba, ColorEncoding::HexString), hcl::Value::from("#FF4040FF"));
    assert_eq!(ColorCodec::from_rgba(rgba, ColorEncoding::ByteArray4), hcl::Value::from(vec![255, 64, 64, 255]));
    for encoding in
        [ColorEncoding::PackedRgbaU32, ColorEncoding::FloatArray4, ColorEncoding::ByteArray4, ColorEncoding::HexString]
    {
        assert_eq!(ColorCodec::to_rgba(&ColorCodec::from_rgba(rgba, encoding), encoding), Some(rgba), "{encoding:?}");
    }
    assert_eq!(ColorCodec::to_rgba(&hcl::Value::from("#FF4040"), ColorEncoding::HexString), Some(rgba));
    assert_eq!(ColorCodec::to_rgba(&hcl::Value::from(vec![2.0, 0.0, 0.0, 1.0]), ColorEncoding::FloatArray4), None);
    assert_eq!(ColorCodec::to_rgba(&hcl::Value::from("FF4040FF"), ColorEncoding::HexString), None);
}

#[test]
fn bad_color_takes_the_default() {
    let default = hcl::Value::from(vec![0, 0, 0, 255]);
    let document = HclDocument::new("card {\n  color = [300, 0, 0, 255]\n}\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![color(ColorEncoding::ByteArray4, default.clone())]), document);

    assert_eq!(checked.warnings, vec![ConfigWarning::Defaulted { path: "card.color".into() }]);
    assert_eq!(checked.document.get("card.color"), Some(default));
}

#[test]
fn chord_with_an_unknown_key_name_takes_the_default() {
    let toggle = field("keys.toggle", FieldKind::Chord, hcl::Value::from("Ctrl+Shift+M"));
    let document = HclDocument::new("keys {\n  toggle = \"Ctrl+Bogus\"\n}\n").unwrap();

    let checked = SchemaChecker::check(&section(vec![toggle.clone()]), document);

    assert_eq!(checked.warnings, vec![ConfigWarning::Defaulted { path: "keys.toggle".into() }]);
    assert_eq!(checked.document.get("keys.toggle"), Some(hcl::Value::from("Ctrl+Shift+M")));
    assert_eq!(
        SchemaChecker::check_value(&toggle, &hcl::Value::from("shift+ctrl+k")),
        Ok(hcl::Value::from("shift+ctrl+k"))
    );
}

#[test]
fn an_attribute_where_the_schema_needs_a_block_is_replaced_by_a_block_with_defaults() {
    let document = HclDocument::new("# the map\nmini_map = 3\nother = 1\n").unwrap();
    let schema = section(vec![zoom(), scale()]);

    let checked = SchemaChecker::check(&schema, document);
    let again = SchemaChecker::check(&schema, checked.document.clone());

    assert_eq!(
        checked.warnings,
        vec![
            ConfigWarning::Defaulted { path: "mini_map.zoom".into() },
            ConfigWarning::Defaulted { path: "mini_map.scale".into() },
        ]
    );
    assert_eq!(checked.document.get("mini_map.zoom"), Some(hcl::Value::from(2)));
    assert_eq!(checked.document.get("mini_map.scale"), Some(hcl::Value::from(1.0)));
    assert_eq!(checked.document.get("other"), Some(hcl::Value::from(1)));
    assert!(checked.document.text().starts_with("# the map\nmini_map {"));
    assert!(again.warnings.is_empty());
    assert!(!again.changed());
}
