use super::SamplerKind;

/// One texture upload. The RGBA8 pixels are `MeshBuffers::bytes[byte_offset..byte_offset + byte_len]`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureDelta {
    pub id: u64,
    pub sampler: SamplerKind,
    pub full: bool,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
}
