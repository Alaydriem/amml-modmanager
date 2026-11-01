/// One key press the game must see, queued by `FakeKeys`. `vk` is the game's own code (Shift is 0x10, W is
/// 0x57), not a normalised one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FakeKeyOp {
    pub vk: u32,
    pub down: bool,
}
