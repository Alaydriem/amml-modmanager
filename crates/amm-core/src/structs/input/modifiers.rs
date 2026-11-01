use crate::structs::descriptor::Chord;

/// The modifier keys held when a key event happened.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    const CTRL_KEYS: [u32; 3] = [0x11, 0xA2, 0xA3];
    const SHIFT_KEYS: [u32; 3] = [0x10, 0xA0, 0xA1];
    const ALT_KEYS: [u32; 3] = [0x12, 0xA4, 0xA5];

    /// The chord for `vk` pressed with these modifiers. A modifier key does not count as its own
    /// modifier, so Ctrl pressed alone is the chord `Ctrl`.
    pub fn chord_for(&self, vk: u32) -> Chord {
        Chord {
            key: vk,
            ctrl: self.ctrl && !Self::CTRL_KEYS.contains(&vk),
            shift: self.shift && !Self::SHIFT_KEYS.contains(&vk),
            alt: self.alt && !Self::ALT_KEYS.contains(&vk),
            f3: false,
        }
    }
}
