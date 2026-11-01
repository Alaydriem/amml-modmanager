use amm_core::structs::frame::{FrameInputData, LayerKind};

/// What a mod's frame handler receives for one call: the routed input of this frame and the one layer the call
/// draws. A mod that holds both Hud and Overlay is called once per layer, each with its own egui context and its
/// own events, and draws only that layer's content.
#[derive(Debug, Clone, Copy)]
pub struct FrameInputView<'a> {
    data: FrameInputData<'a>,
    layer: LayerKind,
}

impl<'a> FrameInputView<'a> {
    pub fn new(data: FrameInputData<'a>, layer: LayerKind) -> Self {
        Self { data, layer }
    }

    /// The layer this call draws.
    pub fn layer(&self) -> LayerKind {
        self.layer
    }

    /// The routed input of this call: screen size, scale, time, this layer's events and the input flags.
    pub fn data(&self) -> &FrameInputData<'a> {
        &self.data
    }
}
