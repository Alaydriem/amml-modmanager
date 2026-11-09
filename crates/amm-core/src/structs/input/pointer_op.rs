use super::{PointerButton, TurnAxis};

/// One fake mouse action for the game: a button goes down or up, the wheel turns by `steps` notches (positive away
/// from the player), or the look turns by `millidegrees` on one axis. Queued by `FakePointer`, sent by a mod with
/// `send_pointer`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerOp {
    Button { button: PointerButton, down: bool },
    Wheel { steps: i32 },
    Turn { axis: TurnAxis, millidegrees: i32 },
}
