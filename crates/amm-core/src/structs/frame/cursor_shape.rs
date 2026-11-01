/// The cursor the host shows over a layer.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Default,
    Arrow,
    Hand,
    Text,
}
