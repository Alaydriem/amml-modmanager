use crate::services::settings::ChordText;
use crate::structs::descriptor::{Chord, Descriptor};
use crate::structs::registry::{ModIndex, ModMask};
use crate::structs::settings::HclDocument;

/// The chord each registered keybind uses now: the chord stored at its `config_path`, or its default when that
/// value is missing or not a chord. The buffer is reused across calls and holds at most `CAPACITY` entries,
/// the most mods times the most keybinds per mod; keybinds past the bound are not collected.
pub struct EffectiveKeys {
    keys: Vec<(ModIndex, u16, Chord)>,
}

impl EffectiveKeys {
    pub const CAPACITY: usize = ModMask::CAPACITY * 128;

    pub fn new() -> Self {
        Self { keys: Vec::with_capacity(Self::CAPACITY) }
    }

    /// Each entry is the mod, the keybind's index in its descriptor, and the chord.
    pub fn collect<'a>(
        &mut self,
        mods: impl Iterator<Item = (ModIndex, &'a Descriptor, &'a HclDocument)>,
    ) -> &[(ModIndex, u16, Chord)] {
        self.keys.clear();
        for (index, descriptor, document) in mods {
            for (position, keybind) in descriptor.keybinds.iter().enumerate() {
                let Ok(position) = u16::try_from(position) else {
                    break;
                };
                if self.keys.len() == Self::CAPACITY {
                    return &self.keys;
                }
                let stored = document.get(&keybind.config_path);
                let chord = stored.as_ref().and_then(hcl::Value::as_str).and_then(ChordText::parse);
                self.keys.push((index, position, chord.unwrap_or(keybind.default_chord)));
            }
        }
        &self.keys
    }
}

impl Default for EffectiveKeys {
    fn default() -> Self {
        Self::new()
    }
}
