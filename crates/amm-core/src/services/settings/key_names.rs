/// The one table of key names, shared by chord text and the settings panel.
pub struct KeyNames;

impl KeyNames {
    const TABLE: [(&'static str, u32); 89] = [
        ("A", 0x41),
        ("B", 0x42),
        ("C", 0x43),
        ("D", 0x44),
        ("E", 0x45),
        ("F", 0x46),
        ("G", 0x47),
        ("H", 0x48),
        ("I", 0x49),
        ("J", 0x4A),
        ("K", 0x4B),
        ("L", 0x4C),
        ("M", 0x4D),
        ("N", 0x4E),
        ("O", 0x4F),
        ("P", 0x50),
        ("Q", 0x51),
        ("R", 0x52),
        ("S", 0x53),
        ("T", 0x54),
        ("U", 0x55),
        ("V", 0x56),
        ("W", 0x57),
        ("X", 0x58),
        ("Y", 0x59),
        ("Z", 0x5A),
        ("0", 0x30),
        ("1", 0x31),
        ("2", 0x32),
        ("3", 0x33),
        ("4", 0x34),
        ("5", 0x35),
        ("6", 0x36),
        ("7", 0x37),
        ("8", 0x38),
        ("9", 0x39),
        ("F1", 0x70),
        ("F2", 0x71),
        ("F3", 0x72),
        ("F4", 0x73),
        ("F5", 0x74),
        ("F6", 0x75),
        ("F7", 0x76),
        ("F8", 0x77),
        ("F9", 0x78),
        ("F10", 0x79),
        ("F11", 0x7A),
        ("F12", 0x7B),
        ("F13", 0x7C),
        ("F14", 0x7D),
        ("F15", 0x7E),
        ("F16", 0x7F),
        ("F17", 0x80),
        ("F18", 0x81),
        ("F19", 0x82),
        ("F20", 0x83),
        ("F21", 0x84),
        ("F22", 0x85),
        ("F23", 0x86),
        ("F24", 0x87),
        ("Space", 0x20),
        ("Enter", 0x0D),
        ("Escape", 0x1B),
        ("Tab", 0x09),
        ("Backspace", 0x08),
        ("Delete", 0x2E),
        ("Insert", 0x2D),
        ("Home", 0x24),
        ("End", 0x23),
        ("PageUp", 0x21),
        ("PageDown", 0x22),
        ("Left", 0x25),
        ("Right", 0x27),
        ("Up", 0x26),
        ("Down", 0x28),
        ("LeftBracket", 0xDB),
        ("RightBracket", 0xDD),
        ("Equals", 0xBB),
        ("Minus", 0xBD),
        ("Comma", 0xBC),
        ("Period", 0xBE),
        ("Slash", 0xBF),
        ("Backslash", 0xDC),
        ("Semicolon", 0xBA),
        ("Quote", 0xDE),
        ("Grave", 0xC0),
        ("Ctrl", 0x11),
        ("Shift", 0x10),
        ("Alt", 0x12),
    ];

    const HEX_PREFIX: &'static str = "VK_0x";

    /// The name of a virtual key, or `None` when the key has no name in the table.
    pub fn name(vk: u32) -> Option<&'static str> {
        Self::TABLE.iter().find(|(_, code)| *code == vk).map(|(name, _)| *name)
    }

    /// The virtual key for a name, ignoring case. Accepts the `VK_0x..` form for unnamed keys.
    pub fn vk(name: &str) -> Option<u32> {
        if let Some((_, code)) = Self::TABLE.iter().find(|(known, _)| known.eq_ignore_ascii_case(name)) {
            return Some(*code);
        }
        Self::parse_hex_name(name)
    }

    fn parse_hex_name(name: &str) -> Option<u32> {
        let prefix_len = Self::HEX_PREFIX.len();
        let prefix = name.get(..prefix_len)?;
        if !prefix.eq_ignore_ascii_case(Self::HEX_PREFIX) {
            return None;
        }
        let digits = &name[prefix_len..];
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok()
    }
}
