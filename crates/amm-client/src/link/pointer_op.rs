use amml_api::{Look, MouseButton};

/// One mouse action a mod injects through its link's input lease: a button goes down or up, the wheel turns by
/// `notches` (positive away from the player; a fraction waits for the next turn), or the view turns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerOp {
    Button { button: MouseButton, down: bool },
    Wheel { notches: f32 },
    Turn(Look),
}
