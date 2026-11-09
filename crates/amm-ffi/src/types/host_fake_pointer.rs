/// One fake mouse action for the game, in the `op` and `value` numbers of `send_pointer`: `op` 1 left, 2 right,
/// 3 middle button with `value` 1 down or 0 up; `op` 4 the wheel with `value` signed notches.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmHostFakePointer {
    pub op: u32,
    pub value: i32,
}
