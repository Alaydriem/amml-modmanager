use super::UiEvent;

/// What one mod's egui pass receives for a frame.
#[derive(Debug, Clone, Copy)]
pub struct FrameInputData<'a> {
    pub width: u32,
    pub height: u32,
    pub pixels_per_point: f32,
    pub time_secs: f64,
    pub events: &'a [UiEvent],
    pub allowed_layers: u8,
    pub input_flags: u8,
}

impl FrameInputData<'_> {
    /// Bit in `allowed_layers`: the mod may draw a Hud layer this frame.
    pub const LAYER_HUD: u8 = 1;
    /// Bit in `allowed_layers`: the mod may draw an Overlay layer this frame.
    pub const LAYER_OVERLAY: u8 = 2;
    /// Bit in `input_flags`: pointer and wheel events are delivered.
    pub const ALLOW_POINTER: u8 = 1;
    /// Bit in `input_flags`: key and text events are delivered.
    pub const ALLOW_KEYBOARD: u8 = 2;
    /// Bit in `input_flags`: Tab, Enter, Space and the arrows are delivered. Without it the safe key map applies.
    pub const ALLOW_NAV_KEYS: u8 = 4;
}
