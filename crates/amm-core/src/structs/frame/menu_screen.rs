/// The game screen on top, as far as the menu button cares: the start (main menu) screen, the pause
/// screen, or any other screen and play.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum MenuScreen {
    #[default]
    None,
    Start,
    Pause,
}

impl MenuScreen {
    /// The screen for the host's code: 1 start, 2 pause, anything else none.
    pub fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Start,
            2 => Self::Pause,
            _ => Self::None,
        }
    }
}
