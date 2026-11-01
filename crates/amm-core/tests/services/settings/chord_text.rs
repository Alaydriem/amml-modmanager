use amm_core::services::settings::{ChordText, KeyNames};
use amm_core::structs::descriptor::Chord;

#[test]
fn chord_text_parses_modifiers_in_any_order_and_formats_canonically() {
    let expected = Chord { key: 0x4D, ctrl: true, shift: true, alt: false, f3: false };

    assert_eq!(ChordText::parse("Ctrl+Shift+M"), Some(expected));
    assert_eq!(ChordText::parse("shift+CTRL+m"), Some(expected));
    assert_eq!(ChordText::format(expected), "Ctrl+Shift+M");

    let all = Chord { key: 0x70, ctrl: true, shift: true, alt: true, f3: false };
    assert_eq!(ChordText::format(all), "Ctrl+Shift+Alt+F1");
    assert_eq!(ChordText::parse("Alt+Shift+Ctrl+F1"), Some(all));
    assert_eq!(ChordText::parse("Alt+F1+Ctrl"), None);
}

#[test]
fn keys_name_each_cap_in_canonical_order() {
    let chord = Chord { key: 0x79, ctrl: true, shift: true, alt: false, f3: false };
    let unnamed = Chord { key: 0xE9, ctrl: false, shift: false, alt: true, f3: false };

    assert_eq!(ChordText::keys(chord), ["Ctrl", "Shift", "F10"]);
    assert_eq!(ChordText::keys(unnamed), ["Alt", "VK_0xE9"]);
}

#[test]
fn modifier_alone_is_a_chord() {
    let ctrl = ChordText::parse("Ctrl");

    assert_eq!(ctrl, Some(Chord { key: 0x11, ctrl: false, shift: false, alt: false, f3: false }));
    assert_eq!(ctrl.map(ChordText::format), Some("Ctrl".to_string()));
}

#[test]
fn unnamed_vk_round_trips_as_hex_name() {
    let chord = Chord { key: 0xE9, ctrl: false, shift: false, alt: true, f3: false };

    assert_eq!(KeyNames::name(0xE9), None);
    assert_eq!(ChordText::format(chord), "Alt+VK_0xE9");
    assert_eq!(ChordText::parse("Alt+VK_0xE9"), Some(chord));
    assert_eq!(ChordText::parse("alt+vk_0xe9"), Some(chord));
}

#[test]
fn unknown_key_name_does_not_parse() {
    assert_eq!(KeyNames::vk("Banana"), None);
    assert_eq!(ChordText::parse("Ctrl+Banana"), None);
    assert_eq!(ChordText::parse(""), None);
    assert_eq!(ChordText::parse("Ctrl+"), None);
    assert_eq!(ChordText::parse("Ctrl+Ctrl+M"), None);
    assert_eq!(ChordText::parse("VK_0x"), None);
    assert_eq!(ChordText::parse("VK_0xZZ"), None);
}

#[test]
fn f3_is_a_modifier_in_canonical_order() {
    let chord = Chord { key: 0x47, ctrl: true, shift: false, alt: false, f3: true };

    assert_eq!(ChordText::parse("F3+G"), Some(Chord { ctrl: false, ..chord }));
    assert_eq!(ChordText::parse("g+f3+ctrl"), None);
    assert_eq!(ChordText::parse("f3+ctrl+g"), Some(chord));
    assert_eq!(ChordText::format(chord), "Ctrl+F3+G");
    assert_eq!(ChordText::keys(chord), ["Ctrl", "F3", "G"]);
}

#[test]
fn f3_is_never_the_key() {
    assert_eq!(ChordText::parse("F3"), None);
    assert_eq!(ChordText::parse("Ctrl+F3"), None);
    assert_eq!(ChordText::parse("F3+F3"), None);
}

#[test]
fn with_f3_never_marks_f3_itself() {
    let g = Chord { key: 0x47, ctrl: false, shift: false, alt: false, f3: false };
    let f3 = Chord { key: Chord::F3, ..g };

    assert!(g.with_f3(true).f3);
    assert!(!g.with_f3(false).f3);
    assert!(!f3.with_f3(true).f3);
}
