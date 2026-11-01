use amm_core::structs::frame::CursorShape;
use egui::Rect;

/// What a mod's frame handler reports besides what it painted. `layer_rect` is in egui points; the bridge
/// converts it to physical pixels for the host.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameReply {
    pub cursor: CursorShape,
    pub layer_rect: Rect,
}
