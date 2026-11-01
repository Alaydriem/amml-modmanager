use super::AmmUiEventKind;

/// One routed input event. Positions are physical pixels.
///
/// `PointerButton` carries the button index in `key`. `Wheel` carries the line delta in `x` and `y`.
/// `Key` carries the virtual key in `key`. `Char` carries the code point in `ch`.
/// `modifiers` is a mask: 1 Ctrl, 2 Shift, 4 Alt.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmUiEvent {
    pub kind: AmmUiEventKind,
    pub x: f32,
    pub y: f32,
    pub key: u32,
    pub modifiers: u8,
    pub down: bool,
    pub ch: u32,
}
