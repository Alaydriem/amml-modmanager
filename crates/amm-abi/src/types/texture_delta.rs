/// One texture upload. The RGBA8 pixels are `bytes[byte_offset..byte_offset + byte_len]` of the mesh view.
/// `sampler` is an `AmmSamplerKind` value and `full` is 0 or 1. The host rejects any other value.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmTextureDelta {
    pub id: u64,
    pub sampler: u8,
    pub full: u8,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub byte_offset: u32,
    pub byte_len: u32,
}
