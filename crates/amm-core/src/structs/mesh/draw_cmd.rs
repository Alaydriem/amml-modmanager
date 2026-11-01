use super::SamplerKind;

/// One indexed draw. `clip` is `[min_x, min_y, max_x, max_y]` in physical pixels.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawCmd {
    pub texture: u64,
    pub index_offset: u32,
    pub index_count: u32,
    pub clip: [f32; 4],
    pub shader: u8,
    pub sampler: SamplerKind,
}

impl DrawCmd {
    /// Pixel shader for egui-managed textures.
    pub const SHADER_EGUI: u8 = 0;
    /// Pixel shader for user textures such as map tiles.
    pub const SHADER_USER: u8 = 1;
    /// Pixel shader for the game picture: its colour is drawn opaque, since the alpha of a back buffer means
    /// nothing.
    pub const SHADER_PICTURE: u8 = 2;
    /// Set on `texture` for user textures so they never collide with managed ids.
    pub const USER_TEXTURE_FLAG: u64 = 1 << 63;
}
