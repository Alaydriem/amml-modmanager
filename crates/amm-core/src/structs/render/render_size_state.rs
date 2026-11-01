use super::RenderSize;

/// What a `RenderSize` holder reads back: the size it forces (`None` when it forces none) and the back buffer of
/// the last Present, its size in physical pixels and its DXGI format. A forced size is in effect once `actual`
/// equals `wanted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderSizeState {
    pub wanted: Option<RenderSize>,
    pub actual: RenderSize,
    pub format: u32,
}
