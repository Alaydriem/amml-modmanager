use crate::services::settings::KeyNames;
use crate::structs::descriptor::Chord;

/// Reads and writes chords as text such as `Ctrl+Shift+M`.
pub struct ChordText;

impl ChordText {
    const CTRL: &'static str = "Ctrl";
    const SHIFT: &'static str = "Shift";
    const ALT: &'static str = "Alt";
    const F3: &'static str = "F3";

    /// Parses `+` separated text. The last part is the key, the parts before it are modifiers in any order.
    /// F3 is only ever a modifier, so a chord whose key is F3 does not parse.
    pub fn parse(text: &str) -> Option<Chord> {
        let (modifiers, key_name) = match text.rsplit_once('+') {
            Some((modifiers, key_name)) => (Some(modifiers), key_name),
            None => (None, text),
        };
        let key = KeyNames::vk(key_name).filter(|key| *key != Chord::F3)?;
        let mut chord = Chord { key, ctrl: false, shift: false, alt: false, f3: false };
        let Some(modifiers) = modifiers else {
            return Some(chord);
        };
        for part in modifiers.split('+') {
            let flag = if part.eq_ignore_ascii_case(Self::CTRL) {
                &mut chord.ctrl
            } else if part.eq_ignore_ascii_case(Self::SHIFT) {
                &mut chord.shift
            } else if part.eq_ignore_ascii_case(Self::ALT) {
                &mut chord.alt
            } else if part.eq_ignore_ascii_case(Self::F3) {
                &mut chord.f3
            } else {
                return None;
            };
            if *flag {
                return None;
            }
            *flag = true;
        }
        Some(chord)
    }

    /// Writes the canonical form: Ctrl, Shift, Alt, F3, then the key.
    pub fn format(chord: Chord) -> String {
        Self::keys(chord).join("+")
    }

    /// The name of each key of the chord in the canonical order, one per key cap.
    pub fn keys(chord: Chord) -> Vec<String> {
        let mut keys = Vec::new();
        for (is_set, name) in
            [(chord.ctrl, Self::CTRL), (chord.shift, Self::SHIFT), (chord.alt, Self::ALT), (chord.f3, Self::F3)]
        {
            if is_set {
                keys.push(name.to_owned());
            }
        }
        keys.push(match KeyNames::name(chord.key) {
            Some(name) => name.to_owned(),
            None => format!("VK_0x{:X}", chord.key),
        });
        keys
    }
}
