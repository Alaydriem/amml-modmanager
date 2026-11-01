use std::io::Cursor;

use png::{ColorType, Decoder, Limits, Transformations};

use crate::errors::IconError;
use crate::structs::icon::ModIcon;

/// Decodes untrusted PNG bytes into a bounded RGBA8 icon.
///
/// The byte cap and the header size are checked before any pixel buffer is made.
/// Only the first frame is read, and no metadata is kept.
pub struct IconDecoder;

impl IconDecoder {
    pub const MAX_BYTES: usize = 262_144;
    pub const MAX_SIDE: u32 = 256;
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    pub fn decode(bytes: &[u8]) -> Result<ModIcon, IconError> {
        if bytes.len() > Self::MAX_BYTES {
            return Err(IconError::TooLarge { len: bytes.len() });
        }

        if !bytes.starts_with(&Self::SIGNATURE) {
            return Err(IconError::NotPng);
        }

        let (width, height) = Self::header_size(bytes)?;
        if width == 0 || height == 0 || width > Self::MAX_SIDE || height > Self::MAX_SIDE {
            return Err(IconError::BadSize { width, height });
        }

        let side = Self::MAX_SIDE as usize;
        let mut decoder = Decoder::new_with_limits(Cursor::new(bytes), Limits { bytes: side * side * 8 });
        decoder.set_transformations(Transformations::EXPAND | Transformations::STRIP_16 | Transformations::ALPHA);
        decoder.set_ignore_text_chunk(true);
        decoder.set_ignore_iccp_chunk(true);
        decoder.ignore_checksums(false);

        let mut reader = decoder.read_info().map_err(|_| IconError::Corrupt)?;

        let size = reader.output_buffer_size().ok_or(IconError::Corrupt)?;
        let mut buffer = vec![0u8; size];
        let frame = reader.next_frame(&mut buffer).map_err(|_| IconError::Corrupt)?;
        let used = frame.buffer_size();
        buffer.truncate(used);

        let pixels = width as usize * height as usize;
        let rgba = match frame.color_type {
            ColorType::Rgba => buffer,
            ColorType::GrayscaleAlpha => {
                let mut out = Vec::with_capacity(pixels * 4);
                for px in buffer.as_chunks::<2>().0.iter() {
                    out.extend_from_slice(&[px[0], px[0], px[0], px[1]]);
                }
                out
            }
            _ => return Err(IconError::Corrupt),
        };
        if rgba.len() != pixels * 4 {
            return Err(IconError::Corrupt);
        }

        Ok(ModIcon { width: width as u16, height: height as u16, rgba })
    }

    // width and height from the IHDR chunk, which must come first
    fn header_size(bytes: &[u8]) -> Result<(u32, u32), IconError> {
        let header = bytes.get(8..24).ok_or(IconError::NotPng)?;
        if &header[4..8] != b"IHDR" {
            return Err(IconError::NotPng);
        }
        let width = u32::from_be_bytes([header[8], header[9], header[10], header[11]]);
        let height = u32::from_be_bytes([header[12], header[13], header[14], header[15]]);
        Ok((width, height))
    }
}
