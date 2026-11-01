use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use egui::Context;

use super::ClipboardHold;

/// A handler that keeps a memory clipboard locked until the handler is dropped.
pub struct HoldingHandler<H: FrameHandler> {
    inner: H,
    _hold: ClipboardHold,
}

impl<H: FrameHandler> HoldingHandler<H> {
    pub fn new(inner: H, hold: ClipboardHold) -> Self {
        Self { inner, _hold: hold }
    }
}

impl<H: FrameHandler> FrameHandler for HoldingHandler<H> {
    fn frame(&mut self, ctx: &Context, input: &FrameInputView) -> FrameReply {
        self.inner.frame(ctx, input)
    }
}
