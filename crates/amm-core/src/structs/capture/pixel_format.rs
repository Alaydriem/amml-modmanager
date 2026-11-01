/// A back buffer format the screenshot writer reads, named by its DXGI format code. Every format holds 4 bytes
/// per pixel. Any other DXGI code (for example an HDR `R16G16B16A16_FLOAT` buffer) has no `PixelFormat`, and a
/// capture in it is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    /// `R8G8B8A8_UNORM` and `R8G8B8A8_UNORM_SRGB`: bytes red, green, blue, alpha.
    Rgba8,
    /// `B8G8R8A8_UNORM` and `B8G8R8A8_UNORM_SRGB`: bytes blue, green, red, alpha.
    Bgra8,
    /// `R10G10B10A2_UNORM`: a little-endian 32-bit word, red in bits 0 to 9, green in 10 to 19, blue in 20 to 29.
    Rgb10A2,
}

impl PixelFormat {
    pub const DXGI_R10G10B10A2_UNORM: u32 = 24;
    pub const DXGI_R8G8B8A8_UNORM: u32 = 28;
    pub const DXGI_R8G8B8A8_UNORM_SRGB: u32 = 29;
    pub const DXGI_B8G8R8A8_UNORM: u32 = 87;
    pub const DXGI_B8G8R8A8_UNORM_SRGB: u32 = 91;
    pub const BYTES_PER_PIXEL: u32 = 4;

    /// The format of a DXGI format code, or `None` when the writer does not read it.
    pub fn from_dxgi(code: u32) -> Option<PixelFormat> {
        match code {
            Self::DXGI_R8G8B8A8_UNORM | Self::DXGI_R8G8B8A8_UNORM_SRGB => Some(Self::Rgba8),
            Self::DXGI_B8G8R8A8_UNORM | Self::DXGI_B8G8R8A8_UNORM_SRGB => Some(Self::Bgra8),
            Self::DXGI_R10G10B10A2_UNORM => Some(Self::Rgb10A2),
            _ => None,
        }
    }
}
