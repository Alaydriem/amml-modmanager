/// What a `UiEvent` reports.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiEventKind {
    PointerMove,
    PointerButton,
    Wheel,
    Key,
    Char,
    FocusLost,
}
