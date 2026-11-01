use amm_overlay::PointerButton;

use crate::traits::GameCalls;

/// One event of the game's mouse feed (`MouseDevice::feed`). Input thread.
pub struct MouseInput;

impl MouseInput {
    const MOVE: u8 = 0;
    const LEFT: u8 = 1;
    const RIGHT: u8 = 2;
    const MIDDLE: u8 = 3;
    const WHEEL: u8 = 4;

    /// True when the game must not see the event. A movement goes to the host as a raw delta. A button or the
    /// wheel is cancelled when the window copy of it was consumed. While the real cursor is inside a game
    /// viewport, `x` and `y` become the point of the full window it stands for.
    pub fn feed(calls: &dyn GameCalls, action: u8, x: &mut i16, y: &mut i16, dx: i16, dy: i16) -> bool {
        if let Some((game_x, game_y)) = calls.game_pointer() {
            *x = game_x;
            *y = game_y;
        }
        match action {
            Self::MOVE => calls.host_pointer_delta(f32::from(dx), f32::from(dy)),
            Self::LEFT => calls.button_consumed(PointerButton::Left),
            Self::RIGHT => calls.button_consumed(PointerButton::Right),
            Self::MIDDLE => calls.button_consumed(PointerButton::Middle),
            Self::WHEEL => calls.button_consumed(PointerButton::Wheel),
            _ => false,
        }
    }
}
