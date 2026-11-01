use png::{BitDepth, ColorType, Encoder};

/// A generated PNG icon.
pub struct IconPng;

impl IconPng {
    const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

    /// An opaque RGBA PNG of `side` by `side` pixels. The colour channels carry four bits of noise, so the
    /// compressed size stays under the icon input limit at 256 px. Each colour channel has four bits of noise.
    ///
    /// An encode failure returns an empty vector; the bench asserts the decode succeeds before timing.
    pub fn square(side: u32) -> Vec<u8> {
        let mut state = Self::SEED;
        let mut pixels = Vec::with_capacity(side as usize * side as usize * 4);
        for _ in 0..side * side {
            for _ in 0..3 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                pixels.push(0x80 | ((((state >> 32) as u8) & 0x0F) << 3));
            }
            pixels.push(u8::MAX);
        }
        Self::encode(side, &pixels).unwrap_or_default()
    }

    fn encode(side: u32, pixels: &[u8]) -> Result<Vec<u8>, png::EncodingError> {
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, side, side);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.write_header()?.write_image_data(pixels)?;
        Ok(out)
    }
}
