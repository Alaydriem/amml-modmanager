/// A mouse button a mod can hold down for the game with `send_pointer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
}

impl PointerButton {
    /// The button for a pointer button number of `InputEvent::PointerButton`: 0 left, 1 right, 2 middle. `None`
    /// for the side buttons, which the game's mouse feed does not take from a mod.
    pub fn from_number(button: u8) -> Option<Self> {
        match button {
            0 => Some(Self::Left),
            1 => Some(Self::Right),
            2 => Some(Self::Middle),
            _ => None,
        }
    }
}
