use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, PixelFormat};

/// Turns the pixels of a capture into tightly packed 8-bit RGB rows. Alpha is dropped, because the picture is
/// opaque, and the padding at the end of each row is skipped.
pub struct PixelConverter;

impl PixelConverter {
    const BYTES_PER_PIXEL: usize = PixelFormat::BYTES_PER_PIXEL as usize;
    const RGB: usize = 3;
    const TEN_BITS: u32 = 0x3FF;

    /// The RGB bytes of `image`, `width * 3` per row. A format with no `PixelFormat` is `UnsupportedFormat`; an
    /// empty image, a row pitch below `width * 4`, or a buffer shorter than its rows is `BadImage`.
    pub fn to_rgb(image: &CaptureImage) -> Result<Vec<u8>, StoreError> {
        let format = PixelFormat::from_dxgi(image.format).ok_or(StoreError::UnsupportedFormat(image.format))?;
        let (width, height, pitch) = (image.width as usize, image.height as usize, image.row_pitch as usize);
        let row_bytes = width.checked_mul(Self::BYTES_PER_PIXEL).ok_or(StoreError::BadImage)?;
        let needed = height
            .checked_sub(1)
            .and_then(|rows| rows.checked_mul(pitch))
            .and_then(|start| start.checked_add(row_bytes))
            .ok_or(StoreError::BadImage)?;
        if width == 0 || pitch < row_bytes || image.pixels.len() < needed {
            return Err(StoreError::BadImage);
        }
        let mut out = Vec::with_capacity(width * height * Self::RGB);
        for row in image.pixels.chunks(pitch).take(height) {
            let (pixels, _) = row[..row_bytes].as_chunks::<{ Self::BYTES_PER_PIXEL }>();
            for pixel in pixels {
                out.extend_from_slice(&Self::rgb(format, *pixel));
            }
        }
        Ok(out)
    }

    fn rgb(format: PixelFormat, [a, b, c, d]: [u8; 4]) -> [u8; 3] {
        match format {
            PixelFormat::Rgba8 => [a, b, c],
            PixelFormat::Bgra8 => [c, b, a],
            PixelFormat::Rgb10A2 => {
                let word = u32::from_le_bytes([a, b, c, d]);
                [Self::scale(word), Self::scale(word >> 10), Self::scale(word >> 20)]
            }
        }
    }

    // A 10-bit channel scaled to 8 bits, rounded to the nearest value.
    fn scale(word: u32) -> u8 {
        let value = word & Self::TEN_BITS;
        u8::try_from((value * 255 + Self::TEN_BITS / 2) / Self::TEN_BITS).unwrap_or(u8::MAX)
    }
}
