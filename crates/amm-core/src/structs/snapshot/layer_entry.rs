use crate::structs::frame::LayerKind;
use crate::structs::registry::ModIndex;

/// One layer of one mod for the next frame. A mod that holds both Hud and Overlay has two entries.
/// `allowed` is the one bit of `kind`: `FrameInputData::LAYER_HUD` or `FrameInputData::LAYER_OVERLAY`,
/// and the frame callback of that layer gets it as `allowed_layers`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerEntry {
    pub mod_index: ModIndex,
    pub kind: LayerKind,
    pub allowed: u8,
}
