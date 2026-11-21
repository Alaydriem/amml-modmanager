/// What the `FramePresent` listener of a `SurfaceRig` does with the surface it is lent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RigRequest {
    /// `SurfaceRenderer::draw`.
    Draw,
    /// `SurfaceRenderer::draw_mods`.
    DrawMods,
    /// `SurfaceRenderer::draw_manager`.
    DrawManager,
    /// `SurfaceRenderer::refresh_game_picture`.
    RefreshPicture,
}
