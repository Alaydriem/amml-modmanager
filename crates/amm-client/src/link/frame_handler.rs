use amm_core::structs::frame::LayerKind;
use egui::{Context, RawInput};

use super::{FrameInputView, FrameReply};

/// The mod's egui pass. The bridge calls it on the render thread inside the host's frame, once per layer the
/// manager gives the mod in that frame. Each layer has its own egui context.
pub trait FrameHandler {
    /// Paints one pass of the layer `input.layer()` into that layer's `ctx`. `input` is the routed input of
    /// this layer for this frame.
    fn frame(&mut self, ctx: &Context, input: &FrameInputView) -> FrameReply;

    /// Changes the raw input of `layer` after the bridge built it from the host's events and before the egui pass
    /// that calls `frame`. A mod adds or removes events here. The default changes nothing.
    fn adjust_input(&mut self, _layer: LayerKind, _raw: &mut RawInput) {}

    /// The host lost every texture. The bridge has already reset egui's own textures of every layer; a mod that
    /// uploads its own textures sends them again on its next frame.
    fn textures_lost(&mut self) {}

    /// The bridge made a new egui context for a layer, with the amml-ui theme installed: on the layer's first pass,
    /// and again when it replaces the context after lost textures or a panic. Called before the context's first
    /// pass, so a mod that paints with fonts or visuals of its own installs them here.
    fn new_context(&mut self, _ctx: &Context) {}
}
