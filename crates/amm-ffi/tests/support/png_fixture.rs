// test helper: an encoder failure here is a broken fixture, so it panics
#![allow(clippy::unwrap_used)]

use png::{BitDepth, ColorType, Encoder};

pub struct PngFixture;

impl PngFixture {
    // an RGBA8 PNG whose pixels differ per position
    pub fn rgba(width: u32, height: u32) -> Vec<u8> {
        let data: Vec<u8> = (0..width * height).flat_map(|i| [i as u8, (i * 3) as u8, 200, 255]).collect();
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&data).unwrap();
        writer.finish().unwrap();
        out
    }
}
