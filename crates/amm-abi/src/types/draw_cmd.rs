/// One indexed draw. `clip` is `[min_x, min_y, max_x, max_y]` in physical pixels.
/// `sampler` is an `AmmSamplerKind` value. The host rejects any other value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmDrawCmd {
    pub texture: u64,
    pub index_offset: u32,
    pub index_count: u32,
    pub clip: [f32; 4],
    pub shader: u8,
    pub sampler: u8,
}
