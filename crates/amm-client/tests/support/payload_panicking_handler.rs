use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use egui::Context;

use super::PanickingPayload;

/// A handler whose every frame panics with a payload that panics again when it is dropped.
pub struct PayloadPanickingHandler;

impl FrameHandler for PayloadPanickingHandler {
    fn frame(&mut self, _: &Context, _: &FrameInputView) -> FrameReply {
        std::panic::panic_any(PanickingPayload)
    }
}
