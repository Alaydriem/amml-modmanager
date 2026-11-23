use amm_core::structs::render::RenderSize;

use super::FrameCompositor;

/// A Present between `HostRuntime::begin_frame` and `end_frame`: the compositor taken out of the host (`None` when
/// another compose held it), the size message for the game, and whether frame callbacks wait to run.
pub struct FrameStart {
    pub compositor: Option<FrameCompositor>,
    pub message: Option<RenderSize>,
    pub callbacks: bool,
}
