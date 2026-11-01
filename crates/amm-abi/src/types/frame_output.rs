use amm_core::traits::AbiSized;

use super::AmmMeshView;

/// What a mod frame callback fills in. The manager sets `size` to `size_of` its struct; the mod writes only that
/// many bytes and then sets `size` to the bytes it wrote. `layer_rect` is `[min_x, min_y, max_x, max_y]` in physical pixels.
/// `wants_pointer` and `wants_keyboard` are 0 or 1. `wants_keyboard` is 1 only while a text field of the
/// layer has keyboard focus, not for a focused button. `cursor` is an `AmmCursorShape` value.
/// `FrameReplyData::try_from(&AmmFrameOutput)` rejects any other value
/// with `AmmStatus::InvalidArgument`.
///
/// A mod in C or C++ sets the fields of `out` one by one and never assigns a whole struct to `*out`: a manager built
/// for an older minor version gives a smaller `out`, and a whole-struct assignment writes past its end.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmFrameOutput {
    pub size: u32,
    pub mesh: AmmMeshView,
    pub wants_pointer: u8,
    pub wants_keyboard: u8,
    pub cursor: u8,
    pub layer_rect: [f32; 4],
    /// Zero. Fills the struct to its alignment, so a field a later minor version adds always grows `size`.
    pub reserved: [u8; 4],
}

impl AbiSized for AmmFrameOutput {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self {
            size: size_of::<Self>() as u32,
            mesh: AmmMeshView::empty(),
            wants_pointer: 0,
            wants_keyboard: 0,
            cursor: 0,
            layer_rect: [0.0; 4],
            reserved: [0; 4],
        }
    }
}
