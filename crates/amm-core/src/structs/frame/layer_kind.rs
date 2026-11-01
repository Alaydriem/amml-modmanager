use super::FrameInputData;

/// Where a mod's mesh is drawn.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Hud,
    Overlay,
}

impl LayerKind {
    /// The number of layer kinds, the length of a per-layer array.
    pub const COUNT: usize = 2;

    /// The index of this kind in a per-layer array: 0 Hud, 1 Overlay.
    pub fn slot(self) -> usize {
        match self {
            Self::Hud => 0,
            Self::Overlay => 1,
        }
    }

    /// The layer a frame call draws, from its `allowed_layers` bits. An ABI 1.1 host sets one bit. An ABI 1.0
    /// host can set both for one mesh, which it draws at the higher kind, so the Overlay bit wins.
    pub fn from_bits(allowed_layers: u8) -> Self {
        if allowed_layers & FrameInputData::LAYER_OVERLAY != 0 { Self::Overlay } else { Self::Hud }
    }
}
