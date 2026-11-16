/// What one move intent writes into the game's `RawMoveInputComponent` after the game filled it: the flag word
/// becomes `(flags & !clear) | set`, and `vector` goes to both the analog move vector and the raw move. The vector
/// is `[left, forward]`, the order of the game's Vec2.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveWrite {
    pub clear: u32,
    pub set: u32,
    pub vector: [f32; 2],
}
