use amm_overlay::DrawReport;

/// What the `FramePresent` listener of a `SurfaceRig` saw in one call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RigOutcome {
    /// The event lent no surface.
    NoSurface,
    Drawn(DrawReport),
    /// The result of `refresh_game_picture`, the error as its text.
    Picture(Result<(), String>),
}
