use amm_core::traits::AbiSized;

use super::AmmUiEvent;

/// What one mod egui pass receives. `events` stays valid for the duration of the frame callback.
///
/// `size` is `size_of` the struct the manager was built with; the mod reads only that many bytes.
/// `allowed_layers`: bit 0 Hud, bit 1 Overlay. The host calls the frame callback once per layer of
/// the mod in each frame, and exactly one bit is set: the layer this call draws. A mod that holds both Hud and
/// Overlay is called twice, Hud first, and returns a separate mesh each time.
/// `input_flags`: bit 0 pointer, bit 1 keyboard, bit 2 navigation keys.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmFrameInput {
    pub size: u32,
    pub width: u32,
    pub height: u32,
    pub pixels_per_point: f32,
    pub time_secs: f64,
    pub events: *const AmmUiEvent,
    pub event_count: usize,
    pub allowed_layers: u8,
    pub input_flags: u8,
    /// Zero. Fills the struct to its alignment, so a field a later minor version adds always grows `size`.
    pub reserved: [u8; 6],
}

impl AbiSized for AmmFrameInput {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            width: 0,
            height: 0,
            pixels_per_point: 0.0,
            time_secs: 0.0,
            events: std::ptr::null(),
            event_count: 0,
            allowed_layers: 0,
            input_flags: 0,
            reserved: [0; 6],
        }
    }
}
