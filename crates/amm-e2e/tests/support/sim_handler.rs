use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::CursorShape;
use egui::{Color32, Context, LayerId, Rect, pos2};

/// What a simulated mod does in its frame callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SimBehavior {
    Draw,
    Panic,
}

/// A frame handler that counts its calls and then draws a rectangle or panics.
pub struct SimHandler {
    behavior: SimBehavior,
    frames: Arc<AtomicU32>,
}

impl SimHandler {
    pub fn new(behavior: SimBehavior, frames: Arc<AtomicU32>) -> Self {
        Self { behavior, frames }
    }
}

impl FrameHandler for SimHandler {
    fn frame(&mut self, ctx: &Context, _: &FrameInputView) -> FrameReply {
        self.frames.fetch_add(1, Ordering::SeqCst);
        if self.behavior == SimBehavior::Panic {
            panic!("simulated frame failure");
        }
        let rect = Rect::from_min_max(pos2(10.0, 10.0), pos2(110.0, 60.0));
        ctx.layer_painter(LayerId::background()).rect_filled(rect, 0.0, Color32::RED);
        FrameReply { cursor: CursorShape::Default, layer_rect: rect }
    }
}
