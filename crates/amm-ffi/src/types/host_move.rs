/// The held move intent for the game, in the numbers of `send_move`: the forward and left axes in thousandths and
/// the jump, sneak and sprint flags.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmHostMove {
    pub forward_milli: i32,
    pub left_milli: i32,
    pub flags: u32,
}
