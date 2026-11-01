/// How the host samples a texture.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmSamplerKind {
    Linear = 0,
    Nearest,
}
