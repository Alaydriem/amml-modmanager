/// A held movement of the local player that a mod asks for with `send_move`. It stays until the mod sends another
/// one or the manager clears it. The all-zero intent is the clear.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct MoveIntent {
    /// -1.0 to 1.0; positive is forward.
    pub forward: f32,
    /// -1.0 to 1.0; positive is to the left, the game's own sign for the side axis.
    pub left: f32,
    /// Jump is held: the player jumps again each time it lands, and swims up in water.
    pub jump: bool,
    /// Sneak is held, whatever the player's toggle option says.
    pub sneak: bool,
    /// Sprint is held, whatever the player's toggle option says.
    pub sprint: bool,
}

impl MoveIntent {
    pub const ZERO: Self = Self { forward: 0.0, left: 0.0, jump: false, sneak: false, sprint: false };

    /// No axis and no action: the clear.
    pub fn is_zero(&self) -> bool {
        self.forward == 0.0 && self.left == 0.0 && !self.jump && !self.sneak && !self.sprint
    }

    /// Both axes are numbers from -1.0 to 1.0.
    pub fn in_range(&self) -> bool {
        Self::axis_in_range(self.forward) && Self::axis_in_range(self.left)
    }

    fn axis_in_range(value: f32) -> bool {
        value.is_finite() && value.abs() <= 1.0
    }
}
