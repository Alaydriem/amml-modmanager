use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::Path;

use amm_core::errors::StoreError;
use amm_core::structs::capture::CaptureImage;

use crate::services::{ConfigFile, PixelConverter};

/// Writes a capture as an opaque 8-bit RGB PNG, marked sRGB. The file is written to `<name>.tmp`, flushed to
/// disk, then renamed over the target, so the target is either the whole new picture or left as it was. On any
/// failure the temporary file is removed. It does not make the parent folder.
pub struct CaptureWriter;

impl CaptureWriter {
    pub fn write(image: &CaptureImage) -> Result<(), StoreError> {
        let rgb = PixelConverter::to_rgb(image)?;
        let tmp = ConfigFile::sibling(&image.path, ".tmp");
        if Self::encode(&tmp, image.width, image.height, &rgb).is_err() {
            let _ = fs::remove_file(&tmp);
            return Err(StoreError::Io { path: tmp });
        }
        fs::rename(&tmp, &image.path).map_err(|_| {
            let _ = fs::remove_file(&tmp);
            StoreError::Io { path: image.path.clone() }
        })
    }

    fn encode(path: &Path, width: u32, height: u32, rgb: &[u8]) -> io::Result<()> {
        let mut out = BufWriter::new(File::create(path)?);
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgb);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_source_srgb(png::SrgbRenderingIntent::Perceptual);
        let mut writer = encoder.write_header().map_err(io::Error::other)?;
        writer.write_image_data(rgb).map_err(io::Error::other)?;
        writer.finish().map_err(io::Error::other)?;
        out.flush()?;
        out.into_inner().map_err(io::IntoInnerError::into_error)?.sync_all()
    }
}
