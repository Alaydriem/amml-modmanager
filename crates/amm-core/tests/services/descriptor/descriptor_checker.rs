use amm_core::errors::DescriptorError;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::structs::descriptor::{Capability, Chord, Descriptor, FieldKind, KeyScope, KeybindDecl, SettingField};

const SPEC_EXAMPLE: &str = r##"
namespace   = "alaydriem"
name        = "demo"
display_name = "Demo"
version      = "0.1.0"
author       = "Alaydriem"
config_path  = "config.hcl"

claim "Camera" {
  mode   = "Shared"
  reason = "Takes the camera while the demo key is held"
}

keybind "take_camera" {
  label       = "Take camera"
  default     = "Ctrl+Shift+C"
  capability  = "Camera"
  scope       = "Global"
  config_path = "keys.take_camera"
}

section "keys" {
  title = "Keys"
  field "keys.take_camera" {
    label   = "Take camera"
    kind    = "chord"
    default = "Ctrl+Shift+C"
    apply   = "Live"
  }
}

section "card" {
  title = "Card"
  field "card.scale" {
    label   = "Scale"
    kind    = { float = { min = 0.0, max = 1.0, step = 0.05 } }
    default = 0.5
    apply   = "Live"
  }
}
"##;

fn checker() -> DescriptorChecker {
    DescriptorChecker::new(128, 256)
}

fn with_fields(fields: &str) -> String {
    format!(
        r##"
namespace   = "alaydriem"
name        = "m"
display_name = "M"
version = "1"
author = "a"
config_path = "config.hcl"

section "s" {{
  title = "S"
{fields}
}}
"##
    )
}

fn field(path: &str, kind: &str, default: &str) -> String {
    format!("field \"{path}\" {{\n label = \"L\"\n kind = {kind}\n default = {default}\n apply = \"Live\"\n}}\n")
}

fn parse_fields(fields: &str) -> Result<Descriptor, DescriptorError> {
    checker().parse(&with_fields(fields))
}

#[test]
fn the_spec_example_descriptor_parses_and_passes() {
    let descriptor = checker().parse(SPEC_EXAMPLE).unwrap();

    assert_eq!(descriptor.id(), "@alaydriem/demo");
    assert_eq!(descriptor.claims.len(), 1);
    assert_eq!(descriptor.claims[0].capability, Capability::Camera);
    let keybind = &descriptor.keybinds[0];
    assert_eq!(keybind.action, "take_camera");
    assert_eq!(keybind.default_chord, Chord { key: 0x43, ctrl: true, shift: true, alt: false, f3: false });
    assert_eq!(keybind.scope, KeyScope::Global);
    assert_eq!(keybind.config_path, "keys.take_camera");
    let scale = &descriptor.settings[1].fields[0];
    assert_eq!(scale.path, "card.scale");
    assert_eq!(scale.kind, FieldKind::Float { min: 0.0, max: 1.0, step: 0.05, unit: None });
}

#[test]
fn a_keybind_on_bare_f3_is_refused() {
    let bare = SPEC_EXAMPLE.replace("Ctrl+Shift+C", "F3");
    let held = SPEC_EXAMPLE.replace("Ctrl+Shift+C", "F3+G");

    assert!(checker().parse(&bare).is_err());
    assert!(checker().parse(&held).is_ok());
}

#[test]
fn rejects_keybind_whose_config_path_is_not_a_chord_field() {
    let text = SPEC_EXAMPLE.replace("config_path = \"keys.take_camera\"", "config_path = \"card.scale\"");

    assert_eq!(
        checker().parse(&text),
        Err(DescriptorError::KeybindPathNotChord { action: "take_camera".into(), path: "card.scale".into() })
    );

    let missing = SPEC_EXAMPLE.replace("config_path = \"keys.take_camera\"", "config_path = \"keys.nowhere\"");
    assert!(matches!(checker().parse(&missing), Err(DescriptorError::KeybindPathNotChord { .. })));
}

#[test]
fn rejects_int_default_outside_its_range() {
    let kind = "{ int = { min = 1, max = 10, step = 1 } }";

    assert_eq!(parse_fields(&field("a.n", kind, "11")), Err(DescriptorError::DefaultOutOfRange { path: "a.n".into() }));
    assert!(parse_fields(&field("a.n", kind, "10")).is_ok());
    assert_eq!(parse_fields(&field("a.n", kind, "1.5")), Err(DescriptorError::DefaultWrongType { path: "a.n".into() }));
}

#[test]
fn rejects_float_default_that_is_a_string() {
    let kind = "{ float = { min = 0.0, max = 1.0, step = 0.1 } }";

    assert_eq!(
        parse_fields(&field("a.f", kind, "\"half\"")),
        Err(DescriptorError::DefaultWrongType { path: "a.f".into() })
    );
    assert_eq!(
        parse_fields(&field("a.f", kind, "1.5")),
        Err(DescriptorError::DefaultOutOfRange { path: "a.f".into() })
    );
}

#[test]
fn rejects_two_claims_on_one_capability() {
    let mut descriptor = checker().parse(SPEC_EXAMPLE).unwrap();
    descriptor.claims.push(descriptor.claims[0].clone());

    assert_eq!(checker().check(&descriptor), Err(DescriptorError::DuplicateClaim(Capability::Camera)));
}

#[test]
fn rejects_two_keybinds_with_one_action_id() {
    let mut descriptor = checker().parse(SPEC_EXAMPLE).unwrap();
    descriptor.keybinds.push(descriptor.keybinds[0].clone());

    assert_eq!(checker().check(&descriptor), Err(DescriptorError::DuplicateAction("take_camera".into())));
}

#[test]
fn rejects_duplicate_setting_paths() {
    let mut descriptor = checker().parse(SPEC_EXAMPLE).unwrap();
    let again = descriptor.settings[1].fields[0].clone();
    descriptor.settings[1].fields.push(again);

    assert_eq!(checker().check(&descriptor), Err(DescriptorError::DuplicatePath("card.scale".into())));
}

#[test]
fn rejects_unparseable_hcl_with_the_parse_reason() {
    match checker().parse("id = ") {
        Err(DescriptorError::Parse(reason)) => assert!(!reason.is_empty()),
        other => panic!("expected Parse, got {other:?}"),
    }
    assert!(matches!(checker().parse("id = \"x\""), Err(DescriptorError::Parse(_))));
}

#[test]
fn rejects_a_path_that_is_not_dotted_identifiers() {
    let kind = "\"bool\"";

    assert_eq!(
        parse_fields(&field("mini map.zoom", kind, "true")),
        Err(DescriptorError::BadPath("mini map.zoom".into()))
    );
    assert_eq!(parse_fields(&field(".zoom", kind, "true")), Err(DescriptorError::BadPath(".zoom".into())));
    assert_eq!(parse_fields(&field("zoom.", kind, "true")), Err(DescriptorError::BadPath("zoom.".into())));
}

#[test]
fn rejects_129_keybinds_and_257_fields() {
    let base = checker().parse(SPEC_EXAMPLE).unwrap();
    let keybind = base.keybinds[0].clone();
    let template = base.settings[1].fields[0].clone();
    let keybinds = |count: usize| -> Vec<KeybindDecl> {
        (0..count)
            .map(|i| KeybindDecl { action: format!("a{i}"), config_path: "keys.take_camera".into(), ..keybind.clone() })
            .collect()
    };
    let fields = |count: usize| -> Vec<SettingField> {
        (0..count).map(|i| SettingField { path: format!("a.f{i}"), ..template.clone() }).collect()
    };

    let mut at_limit = base.clone();
    at_limit.keybinds = keybinds(128);
    assert_eq!(checker().check(&at_limit), Ok(()));
    let mut over = base.clone();
    over.keybinds = keybinds(129);
    assert_eq!(checker().check(&over), Err(DescriptorError::TooManyKeybinds(129)));

    let mut at_limit = base.clone();
    at_limit.keybinds.clear();
    at_limit.settings[0].fields.clear();
    at_limit.settings[1].fields = fields(256);
    assert_eq!(checker().check(&at_limit), Ok(()));
    at_limit.settings[1].fields = fields(257);
    assert_eq!(checker().check(&at_limit), Err(DescriptorError::TooManyFields(257)));
}

#[test]
fn rejects_hide_vanilla_hud_on_a_camera_claim() {
    let text = SPEC_EXAMPLE.replace("mode   = \"Shared\"", "mode   = \"Shared\"\n  hide_vanilla_hud = true");
    assert_eq!(checker().parse(&text), Err(DescriptorError::HideHudOnNonHudClaim));

    let hud = text.replace("claim \"Camera\"", "claim \"Hud\"");
    assert!(checker().parse(&hud).is_ok());
}

#[test]
fn a_block_player_turn_flag_on_a_non_input_claim_is_refused() {
    let text = SPEC_EXAMPLE.replace("mode   = \"Shared\"", "mode   = \"Shared\"\n  block_player_turn = true");
    assert_eq!(checker().parse(&text), Err(DescriptorError::BlockTurnOnNonInputClaim));

    let input = text.replace("claim \"Camera\"", "claim \"Input\"");
    assert!(checker().parse(&input).is_ok());
}

#[test]
fn pass_pointer_motion_parses_on_an_input_claim() {
    let text = SPEC_EXAMPLE.replace("claim \"Camera\"", "claim \"Input\"").replace(
        "mode   = \"Shared\"",
        "mode   = \"Shared\"
  pass_pointer_motion = true",
    );

    let descriptor = checker().parse(&text).expect("parses");

    let claim = descriptor.claims.iter().find(|claim| claim.capability == Capability::Input).expect("input claim");
    assert!(claim.pass_pointer_motion);
}

#[test]
fn full_block_is_read_on_an_exclusive_input_claim() {
    let text = SPEC_EXAMPLE.replace("claim \"Camera\"", "claim \"Input\"").replace(
        "mode   = \"Shared\"",
        "mode   = \"Exclusive\"
  full_block = true",
    );

    let descriptor = checker().parse(&text).expect("parses");

    let claim = descriptor.claims.iter().find(|claim| claim.capability == Capability::Input).expect("input claim");
    assert!(claim.full_block);
}

#[test]
fn full_block_on_another_capability_is_refused() {
    let text = SPEC_EXAMPLE.replace(
        "mode   = \"Shared\"",
        "mode   = \"Exclusive\"
  full_block = true",
    );
    assert_eq!(checker().parse(&text), Err(DescriptorError::FullBlockOnlyOnInput));
}

#[test]
fn full_block_on_a_shared_input_claim_is_refused() {
    let text = SPEC_EXAMPLE.replace("claim \"Camera\"", "claim \"Input\"").replace(
        "mode   = \"Shared\"",
        "mode   = \"Shared\"
  full_block = true",
    );
    assert_eq!(checker().parse(&text), Err(DescriptorError::FullBlockNeedsExclusive));
}

#[test]
fn pass_pointer_motion_on_another_claim_is_refused() {
    let text = SPEC_EXAMPLE.replace(
        "mode   = \"Shared\"",
        "mode   = \"Shared\"
  pass_pointer_motion = true",
    );
    assert_eq!(checker().parse(&text), Err(DescriptorError::PassMotionOnNonInputClaim));
}

#[test]
fn a_claim_without_pass_pointer_motion_parses_as_false() {
    let descriptor = checker().parse(SPEC_EXAMPLE).expect("parses");
    assert!(descriptor.claims.iter().all(|claim| !claim.pass_pointer_motion));
    for fixture in [
        include_str!("../../../fixtures/four_mods/freecam.hcl"),
        include_str!("../../../fixtures/four_mods/dejavu.hcl"),
        include_str!("../../../fixtures/four_mods/minimap.hcl"),
    ] {
        let parsed = checker().parse(fixture).expect("fixture parses");
        assert!(parsed.claims.iter().all(|claim| !claim.pass_pointer_motion));
    }
}

#[test]
fn accepts_every_field_kind_and_color_encoding() {
    let fields = [
        field("k.bool", "\"bool\"", "true"),
        field("k.int", "{ int = { min = -5, max = 5, step = 1 } }", "-5"),
        field("k.float", "{ float = { min = 0.0, max = 2.0, step = 0.1, unit = \"x\" } }", "2.0"),
        field(
            "k.enum",
            "{ enum = { options = [{ value = \"a\", label = \"A\" }, { value = \"b\", label = \"B\" }] } }",
            "\"b\"",
        ),
        field("k.packed", "{ color = { alpha = true, encoding = \"PackedRgbaU32\" } }", "4294967295"),
        field("k.floats", "{ color = { alpha = true, encoding = \"FloatArray4\" } }", "[1.0, 0.25, 0.25, 1.0]"),
        field("k.bytes", "{ color = { alpha = true, encoding = \"ByteArray4\" } }", "[255, 64, 64, 255]"),
        field("k.hex", "{ color = { alpha = true, encoding = \"HexString\" } }", "\"#FF4040FF\""),
        field("k.chord", "\"chord\"", "\"Ctrl+K\""),
    ]
    .concat();

    let descriptor = parse_fields(&fields).unwrap();
    assert_eq!(descriptor.settings[0].fields.len(), 9);

    let bad_enum = field("k.enum", "{ enum = { options = [{ value = \"a\", label = \"A\" }] } }", "\"z\"");
    assert_eq!(parse_fields(&bad_enum), Err(DescriptorError::DefaultOutOfRange { path: "k.enum".into() }));
    let bad_hex = field("k.hex", "{ color = { alpha = true, encoding = \"HexString\" } }", "\"red\"");
    assert_eq!(parse_fields(&bad_hex), Err(DescriptorError::DefaultWrongType { path: "k.hex".into() }));
    let bad_bytes = field("k.b", "{ color = { alpha = true, encoding = \"ByteArray4\" } }", "[256, 0, 0, 0]");
    assert_eq!(parse_fields(&bad_bytes), Err(DescriptorError::DefaultOutOfRange { path: "k.b".into() }));
}

#[test]
fn rejects_two_claim_blocks_with_one_label_in_text() {
    let text = format!("{SPEC_EXAMPLE}\nclaim \"Camera\" {{\n  mode = \"Exclusive\"\n  reason = \"again\"\n}}\n");

    assert_eq!(checker().parse(&text), Err(DescriptorError::DuplicateClaim(Capability::Camera)));
}

#[test]
fn rejects_two_keybind_blocks_with_one_label_in_text() {
    let again = "\nkeybind \"take_camera\" {\n label = \"X\"\n default = \"Ctrl+K\"\n capability = \"Camera\"\n scope = \"Global\"\n config_path = \"keys.take_camera\"\n}\n";
    let text = format!("{SPEC_EXAMPLE}{again}");

    assert_eq!(checker().parse(&text), Err(DescriptorError::DuplicateAction("take_camera".into())));
}

#[test]
fn rejects_two_field_blocks_with_one_path_in_text() {
    let one = field("a.b", "\"bool\"", "true");
    let text = with_fields(&format!("{one}{one}"));

    assert_eq!(checker().parse(&text), Err(DescriptorError::DuplicatePath("a.b".into())));
}

#[test]
fn rejects_two_section_blocks_with_one_id_in_text() {
    let text = format!("{SPEC_EXAMPLE}\nsection \"card\" {{\n  title = \"Again\"\n}}\n");

    assert_eq!(checker().parse(&text), Err(DescriptorError::DuplicateSection("card".into())));
}

#[test]
fn rejects_an_id_with_path_characters() {
    for id in ["../x", "A B", "UPPER", "a/b", &"a".repeat(65)] {
        let text = SPEC_EXAMPLE.replace("\"demo\"", &format!("\"{id}\""));
        assert_eq!(checker().parse(&text), Err(DescriptorError::BadId(id.to_string())), "{id}");
    }
    let empty = SPEC_EXAMPLE.replace("\"demo\"", "\"\"");
    assert_eq!(checker().parse(&empty), Err(DescriptorError::EmptyId));
    assert!(checker().parse(&SPEC_EXAMPLE.replace("\"demo\"", &format!("\"{}\"", "a".repeat(64)))).is_ok());
}

#[test]
fn rejects_a_config_path_that_leaves_the_mod_folder() {
    let valid = checker().parse(SPEC_EXAMPLE).unwrap();
    for path in ["../config.hcl", r"C:\x.hcl", "sub/config.hcl", "config.json", "..hcl", ".hcl", r"a\b.hcl", ""] {
        let descriptor = Descriptor { config_path: path.to_string(), ..valid.clone() };
        assert_eq!(checker().check(&descriptor), Err(DescriptorError::BadConfigPath(path.to_string())), "{path}");
    }
}

#[test]
fn rejects_inverted_ranges_zero_steps_and_empty_enums() {
    let bad = |path: &str, kind: &str, default: &str| {
        assert_eq!(
            parse_fields(&field(path, kind, default)),
            Err(DescriptorError::BadKind { path: path.into() }),
            "{kind}"
        );
    };
    bad("a.i", "{ int = { min = 5, max = 1, step = 1 } }", "3");
    bad("a.i", "{ int = { min = 1, max = 5, step = 0 } }", "3");
    bad("a.f", "{ float = { min = 1.0, max = 0.0, step = 0.1 } }", "0.5");
    bad("a.f", "{ float = { min = 0.0, max = 1.0, step = 0.0 } }", "0.5");
    bad("a.e", "{ enum = { options = [] } }", "\"a\"");
    bad(
        "a.e",
        "{ enum = { options = [{ value = \"a\", label = \"A\" }, { value = \"a\", label = \"B\" }] } }",
        "\"a\"",
    );
}

#[test]
fn unknown_capability_label_reports_one_parse_message() {
    let text = SPEC_EXAMPLE.replace("claim \"Camera\"", "claim \"Teleport\"");
    let message = checker().parse(&text).unwrap_err().to_string();

    assert_eq!(message.matches("descriptor does not parse").count(), 1, "{message}");
    assert!(message.contains("Teleport"), "{message}");
}

#[test]
fn rejects_windows_device_names_in_the_name() {
    let valid = checker().parse(SPEC_EXAMPLE).unwrap();
    for name in ["con", "prn", "aux", "nul", "com1", "com9", "lpt1", "lpt9"] {
        let descriptor = Descriptor { name: name.to_string(), ..valid.clone() };
        let reserved = DescriptorError::ReservedName(format!("@alaydriem/{name}"));
        assert_eq!(checker().check(&descriptor), Err(reserved), "{name}");
    }
    for path in ["con.hcl", "NUL.hcl", "Aux.hcl", "prn.hcl", "COM1.hcl", "lpt3.hcl", "com1.backup.hcl"] {
        let descriptor = Descriptor { config_path: path.to_string(), ..valid.clone() };
        assert_eq!(checker().check(&descriptor), Err(DescriptorError::ReservedName(path.to_string())), "{path}");
    }
    for name in ["console", "com10", "lpt", "nul-mod"] {
        let descriptor = Descriptor { name: name.to_string(), ..valid.clone() };
        assert_eq!(checker().check(&descriptor), Ok(()), "{name}");
    }
    let descriptor = Descriptor { config_path: "connect.hcl".into(), ..valid.clone() };
    assert_eq!(checker().check(&descriptor), Ok(()));
}

#[test]
fn a_zero_or_non_number_budget_is_refused_with_a_reason() {
    let with = |line: &str| format!("{line}\n{SPEC_EXAMPLE}");

    let zero_bytes = checker().parse(&with("texture_budget_mb = 0"));
    let zero_count = checker().parse(&with("texture_budget_count = 0"));
    let text = checker().parse(&with("texture_budget_mb = \"lots\""));
    let negative = checker().parse(&with("texture_budget_count = -4"));
    let fraction = checker().parse(&with("texture_budget_mb = 1.5"));

    assert_eq!(zero_bytes, Err(DescriptorError::ZeroTextureBudget("texture_budget_mb")));
    assert_eq!(zero_count, Err(DescriptorError::ZeroTextureBudget("texture_budget_count")));
    for refused in [text, negative, fraction] {
        match refused {
            Err(DescriptorError::Parse(reason)) => assert!(!reason.is_empty()),
            other => panic!("expected Parse, got {other:?}"),
        }
    }
}

#[test]
fn a_shared_render_size_claim_is_refused() {
    let shared = SPEC_EXAMPLE.replace("claim \"Camera\"", "claim \"RenderSize\"");
    assert_eq!(checker().parse(&shared), Err(DescriptorError::SharedRenderSize));

    let exclusive = shared.replace("mode   = \"Shared\"", "mode   = \"Exclusive\"");
    assert!(checker().parse(&exclusive).is_ok());
}

#[test]
fn a_descriptor_takes_namespace_and_name() {
    let descriptor = checker().parse(SPEC_EXAMPLE).unwrap();

    assert_eq!(descriptor.namespace, "alaydriem");
    assert_eq!(descriptor.name, "demo");
    assert_eq!(descriptor.id(), "@alaydriem/demo");
}

#[test]
fn an_id_attribute_is_refused() {
    let with_id = format!(
        "{SPEC_EXAMPLE}
id = \"alaydriem-demo\"
"
    );

    assert!(matches!(checker().parse(&with_id), Err(DescriptorError::Parse(_))));
}

#[test]
fn a_segment_with_a_slash_or_capital_is_refused() {
    for (namespace, name) in
        [("alay/driem", "demo"), ("alaydriem", "de/mo"), ("Alaydriem", "demo"), ("alaydriem", "Demo")]
    {
        let text = SPEC_EXAMPLE
            .replace("namespace   = \"alaydriem\"", &format!("namespace = \"{namespace}\""))
            .replace("name        = \"demo\"", &format!("name = \"{name}\""));

        assert!(matches!(checker().parse(&text), Err(DescriptorError::BadId(_))), "{namespace}/{name}");
    }
}

#[test]
fn a_segment_over_64_bytes_or_empty_is_refused() {
    let valid = checker().parse(SPEC_EXAMPLE).unwrap();

    let long = Descriptor { name: "a".repeat(65), ..valid.clone() };
    let empty = Descriptor { namespace: String::new(), ..valid.clone() };
    let longest = Descriptor { name: "a".repeat(64), ..valid.clone() };

    assert_eq!(checker().check(&long), Err(DescriptorError::BadId("a".repeat(65))));
    assert_eq!(checker().check(&empty), Err(DescriptorError::EmptyId));
    assert_eq!(checker().check(&longest), Ok(()));
}

#[test]
fn amml_modmanager_is_reserved() {
    let valid = checker().parse(SPEC_EXAMPLE).unwrap();

    let manager = Descriptor { namespace: "amml".into(), name: "modmanager".into(), ..valid.clone() };
    let other_in_namespace = Descriptor { namespace: "amml".into(), name: "other".into(), ..valid.clone() };

    assert_eq!(checker().check(&manager), Err(DescriptorError::ReservedName("@amml/modmanager".into())));
    assert_eq!(checker().check(&other_in_namespace), Ok(()));
}
