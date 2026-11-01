use egui::Key;

/// Windows virtual key codes to egui keys.
pub struct KeyMap;

impl KeyMap {
    const DIGITS: [Key; 10] =
        [Key::Num0, Key::Num1, Key::Num2, Key::Num3, Key::Num4, Key::Num5, Key::Num6, Key::Num7, Key::Num8, Key::Num9];
    const LETTERS: [Key; 26] = [
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
    ];
    const FUNCTION: [Key; 12] =
        [Key::F1, Key::F2, Key::F3, Key::F4, Key::F5, Key::F6, Key::F7, Key::F8, Key::F9, Key::F10, Key::F11, Key::F12];

    /// Every key text fields and dialogs use.
    pub fn to_egui(vk: u32) -> Option<Key> {
        match vk {
            0x09 => Some(Key::Tab),
            0x0D => Some(Key::Enter),
            0x20 => Some(Key::Space),
            0x25 => Some(Key::ArrowLeft),
            0x26 => Some(Key::ArrowUp),
            0x27 => Some(Key::ArrowRight),
            0x28 => Some(Key::ArrowDown),
            _ => Self::to_egui_safe(vk),
        }
    }

    /// The keys that cannot move egui focus or activate the focused widget. Tab, Enter, Space and the
    /// arrows are left out, so an overlay under flight keys is never clicked from the keyboard.
    pub fn to_egui_safe(vk: u32) -> Option<Key> {
        match vk {
            0x08 => Some(Key::Backspace),
            0x1B => Some(Key::Escape),
            0x21 => Some(Key::PageUp),
            0x22 => Some(Key::PageDown),
            0x23 => Some(Key::End),
            0x24 => Some(Key::Home),
            0x2D => Some(Key::Insert),
            0x2E => Some(Key::Delete),
            0x30..=0x39 => Self::DIGITS.get((vk - 0x30) as usize).copied(),
            0x41..=0x5A => Self::LETTERS.get((vk - 0x41) as usize).copied(),
            0x70..=0x7B => Self::FUNCTION.get((vk - 0x70) as usize).copied(),
            _ => None,
        }
    }
}
