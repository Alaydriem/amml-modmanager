/// A back-buffer size in physical pixels that a mod holding `RenderSize` makes the game draw at, whatever the
/// size of its window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct RenderSize {
    pub width: u32,
    pub height: u32,
}

impl RenderSize {
    /// The largest side a forced size may have, the largest 2D texture D3D11 and D3D12 make.
    pub const MAX_SIDE: u32 = 16384;

    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// True for a 0 x 0 (or 0 by anything) size, which a minimised window gives.
    pub fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// True when both sides are 1 to `MAX_SIDE`.
    pub fn is_forcible(&self) -> bool {
        !self.is_empty() && self.width <= Self::MAX_SIDE && self.height <= Self::MAX_SIDE
    }
}
