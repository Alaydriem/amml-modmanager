use crate::structs::capture::ScreenshotLayers;
use crate::structs::registry::ModIndex;

/// One capture the overlay is to take at the next Present, at the capture point `layers` names. Plain data, so a
/// fixed list of them copies without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRequest {
    pub sequence: u32,
    pub mod_index: ModIndex,
    pub layers: ScreenshotLayers,
}

impl CaptureRequest {
    pub fn new(sequence: u32, mod_index: ModIndex, layers: ScreenshotLayers) -> Self {
        Self { sequence, mod_index, layers }
    }
}
