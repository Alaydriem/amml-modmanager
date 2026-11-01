/// A slot of `WndProcRouter::consumed`: a mouse button, or the wheel after them. The numbers of the buttons are the
/// ones the manager uses.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Left = 0,
    Right = 1,
    Middle = 2,
    Wheel = 3,
}

impl PointerButton {
    /// The number of slots.
    pub const COUNT: usize = 4;
}
