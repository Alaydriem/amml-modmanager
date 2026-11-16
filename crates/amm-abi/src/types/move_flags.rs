/// The `flags` bit of `send_move` that holds jump.
pub const AMM_MOVE_JUMP: u32 = 1;
/// The `flags` bit of `send_move` that holds sneak.
pub const AMM_MOVE_SNEAK: u32 = 1 << 1;
/// The `flags` bit of `send_move` that holds sprint.
pub const AMM_MOVE_SPRINT: u32 = 1 << 2;
/// The most thousandths of one `send_move` axis, either way: 1000 is the full axis.
pub const AMM_MOVE_AXIS_MAX: i32 = 1000;
