use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use amm_client::link::{FrameHandler, FrameInputView, FrameReply};
use amm_core::structs::frame::{CursorShape, LayerKind};
use amml_api::Logger;
use egui::{Color32, Context, LayerId, Pos2, Rect, Vec2};

/// The probe's egui pass: one magenta square at a fixed position on the Overlay layer. It counts the frames it
/// draws and logs `probe layer drawn` once at `TARGET_FRAMES`.
pub struct ProbeLayer {
    log: Logger,
    frames: Arc<AtomicU64>,
}

impl ProbeLayer {
    /// Frames the square is drawn before the check passes.
    pub const TARGET_FRAMES: u64 = 60;
    /// The square's top left corner and side, in egui points.
    pub const SQUARE_X: f32 = 64.0;
    pub const SQUARE_Y: f32 = 64.0;
    pub const SQUARE_SIDE: f32 = 96.0;

    pub fn new(log: Logger, frames: Arc<AtomicU64>) -> Self {
        Self { log, frames }
    }
}

impl FrameHandler for ProbeLayer {
    fn frame(&mut self, ctx: &Context, input: &FrameInputView) -> FrameReply {
        if input.layer() != LayerKind::Overlay {
            return FrameReply { cursor: CursorShape::Default, layer_rect: Rect::NOTHING };
        }
        let rect = Rect::from_min_size(Pos2::new(Self::SQUARE_X, Self::SQUARE_Y), Vec2::splat(Self::SQUARE_SIDE));
        ctx.layer_painter(LayerId::background()).rect_filled(rect, 0.0, Color32::MAGENTA);
        let drawn = self.frames.fetch_add(1, Ordering::Relaxed) + 1;
        if drawn == Self::TARGET_FRAMES {
            let scale = input.data().pixels_per_point;
            let square = format!(
                "{},{},{}",
                (Self::SQUARE_X * scale).round(),
                (Self::SQUARE_Y * scale).round(),
                (Self::SQUARE_SIDE * scale).round()
            );
            self.log.info("probe layer drawn", &[("frames", &drawn.to_string()), ("square", &square)]);
        }
        FrameReply { cursor: CursorShape::Default, layer_rect: rect }
    }
}
