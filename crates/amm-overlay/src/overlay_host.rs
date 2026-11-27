use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};

/// What the overlay asks of the manager. Every call comes on the render thread, through `SurfaceRenderer`, inside
/// a `FramePresent` or `SurfaceLost` listener.
///
/// # Safety
///
/// The frame `frame` returns is as `LayerRenderer::draw` requires: its layer pointer and every pointer of each
/// layer's mesh are null or point to their counts of live elements, until the next call of `frame`.
pub unsafe trait OverlayHost: Sync {
    /// True once the game has run its first tick. No frame is drawn and no device object is made before it.
    fn game_ready(&self) -> bool;

    /// The layers to draw on this frame, or `None` when there are none to draw this time.
    fn frame(&self, input: &AmmHostFrameInput) -> Option<&AmmHostFrame>;

    /// The layer textures and the game picture were dropped with the device objects. Called once per loss, before
    /// the next `frame`, so the manager sends its textures again.
    fn textures_lost(&self);

    /// A frame was not drawn: the game is not ready, the device objects could not be made, or `frame` gave `None`.
    /// The host puts the cursor and the vanilla HUD back to the game if a frame had taken them.
    fn frame_skipped(&self);
}
