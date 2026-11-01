/// What an `AmmUiEvent` reports.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmUiEventKind {
    PointerMove = 0,
    PointerButton,
    Wheel,
    Key,
    Char,
    FocusLost,
}
