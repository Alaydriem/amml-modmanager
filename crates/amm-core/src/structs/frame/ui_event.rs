use super::UiEventKind;

/// One routed input event. Positions are physical pixels.
///
/// `PointerButton` carries the button index in `key` (0 primary, 1 secondary, 2 middle).
/// `Wheel` carries the line delta in `x` and `y`. `Key` carries the virtual key in `key`.
/// `Char` carries the code point in `ch`. `modifiers` is a mask of the `MOD_*` bits.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiEvent {
    pub kind: UiEventKind,
    pub x: f32,
    pub y: f32,
    pub key: u32,
    pub modifiers: u8,
    pub down: bool,
    pub ch: u32,
}

impl UiEvent {
    pub const MOD_CTRL: u8 = 1;
    pub const MOD_SHIFT: u8 = 2;
    pub const MOD_ALT: u8 = 4;
}
