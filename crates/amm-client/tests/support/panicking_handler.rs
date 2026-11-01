use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use egui::Context;

/// A handler whose every frame panics.
pub struct PanickingHandler;

impl FrameHandler for PanickingHandler {
    fn frame(&mut self, _: &Context, _: &FrameInputView) -> FrameReply {
        panic!("handler failed on purpose")
    }
}
