/// What one `LayerRenderer::draw` did with the draw commands of its layer range. A skipped command draws nothing
/// and the commands after it still draw.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrawReport {
    /// Commands drawn.
    pub drawn: u32,
    /// Commands whose index range is outside their mesh.
    pub skipped_range: u32,
    /// Commands that name an unknown pixel shader or sampler.
    pub skipped_pipeline: u32,
    /// Commands whose texture is not in the registry, or name the game picture before its first copy.
    pub skipped_texture: u32,
    /// Commands whose clip rectangle is not finite or is empty once cut to the target.
    pub skipped_scissor: u32,
    /// True when the geometry of the range was not drawn at all: past the buffer cap, or a buffer that could not
    /// be made or mapped.
    pub geometry_dropped: bool,
}

impl DrawReport {
    /// Every command of the range that was not drawn.
    pub fn skipped(&self) -> u32 {
        self.skipped_range + self.skipped_pipeline + self.skipped_texture + self.skipped_scissor
    }
}
