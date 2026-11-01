use amm_core::services::settings::EffectiveKeys;
use amm_core::structs::descriptor::{Capability, Chord, Descriptor, KeyScope, KeybindDecl};
use amm_core::structs::registry::ModIndex;
use amm_core::structs::settings::HclDocument;

fn keybind(action: &str, default_chord: Chord, config_path: &str) -> KeybindDecl {
    KeybindDecl {
        action: action.into(),
        label: action.into(),
        default_chord,
        capability: Capability::Hud,
        scope: KeyScope::Global,
        config_path: config_path.into(),
    }
}

fn descriptor(keybinds: Vec<KeybindDecl>) -> Descriptor {
    Descriptor {
        namespace: "test".into(),
        name: "m".into(),
        display_name: "M".into(),
        version: "1".into(),
        author: "a".into(),
        config_path: "config.hcl".into(),
        texture_budget_mb: None,
        texture_budget_count: None,
        claims: Vec::new(),
        keybinds,
        input_passthrough: Vec::new(),
        settings: Vec::new(),
    }
}

const CTRL_M: Chord = Chord { key: 0x4D, ctrl: true, shift: false, alt: false, f3: false };
const K: Chord = Chord { key: 0x4B, ctrl: false, shift: false, alt: false, f3: false };
const F1: Chord = Chord { key: 0x70, ctrl: false, shift: false, alt: false, f3: false };

#[test]
fn effective_keys_read_the_chord_stored_at_each_config_path() {
    let minimap = descriptor(vec![keybind("toggle", CTRL_M, "keys.toggle"), keybind("cycle", K, "keys.cycle")]);
    let document = HclDocument::new("keys {\n  toggle = \"alt+shift+t\"\n}\n").unwrap();
    let mut keys = EffectiveKeys::new();

    let collected = keys.collect([(ModIndex(3), &minimap, &document)].into_iter());

    assert_eq!(
        collected,
        &[(ModIndex(3), 0, Chord { key: 0x54, ctrl: false, shift: true, alt: true, f3: false }), (ModIndex(3), 1, K)]
    );
}

#[test]
fn effective_keys_fall_back_to_the_default_chord_on_a_bad_value() {
    let first = descriptor(vec![
        keybind("unknown_key", CTRL_M, "keys.a"),
        keybind("not_a_string", K, "keys.b"),
        keybind("missing", F1, "keys.c"),
    ]);
    let second = descriptor(vec![keybind("toggle", K, "toggle")]);
    let bad = HclDocument::new("keys {\n  a = \"Ctrl+Bogus\"\n  b = 7\n}\n").unwrap();
    let good = HclDocument::new("toggle = \"F1\"\n").unwrap();
    let mut keys = EffectiveKeys::new();

    let collected = keys.collect([(ModIndex(0), &first, &bad), (ModIndex(1), &second, &good)].into_iter()).to_vec();

    assert_eq!(
        collected,
        vec![(ModIndex(0), 0, CTRL_M), (ModIndex(0), 1, K), (ModIndex(0), 2, F1), (ModIndex(1), 0, F1)]
    );
    let again = keys.collect([(ModIndex(1), &second, &good)].into_iter());
    assert_eq!(again, &[(ModIndex(1), 0, F1)]);
}

#[test]
fn effective_keys_stop_at_their_capacity() {
    let many: Vec<KeybindDecl> =
        (0..EffectiveKeys::CAPACITY + 5).map(|index| keybind(&format!("a{index}"), K, "keys.none")).collect();
    let big = descriptor(many);
    let document = HclDocument::empty();
    let mut keys = EffectiveKeys::new();

    let collected = keys.collect([(ModIndex(0), &big, &document)].into_iter());

    assert_eq!(collected.len(), EffectiveKeys::CAPACITY);
    assert_eq!(collected.last(), Some(&(ModIndex(0), (EffectiveKeys::CAPACITY - 1) as u16, K)));
}
