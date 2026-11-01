use crate::structs::frame::MenuScreen;

/// The game screens a menu entry shows on, as bits: 1 the start screen, 2 the pause screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuScreens(u8);

impl MenuScreens {
    pub const START: MenuScreens = MenuScreens(1);
    pub const PAUSE: MenuScreens = MenuScreens(2);
    pub const BOTH: MenuScreens = MenuScreens(3);

    /// The screens of `bits`; bits other than the start and pause bits are dropped.
    pub fn from_bits(bits: u8) -> Self {
        Self(bits & Self::BOTH.0)
    }

    pub fn bits(self) -> u8 {
        self.0
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn contains(self, screen: MenuScreen) -> bool {
        match screen {
            MenuScreen::Start => self.0 & Self::START.0 != 0,
            MenuScreen::Pause => self.0 & Self::PAUSE.0 != 0,
            MenuScreen::None => false,
        }
    }
}
