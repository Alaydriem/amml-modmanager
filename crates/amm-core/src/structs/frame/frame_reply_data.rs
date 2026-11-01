use super::CursorShape;

/// What a mod frame pass reports back besides its mesh. `layer_rect` is `[min_x, min_y, max_x, max_y]` in
/// physical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameReplyData {
    pub cursor: CursorShape,
    pub wants_pointer: bool,
    pub wants_keyboard: bool,
    pub layer_rect: [f32; 4],
}
