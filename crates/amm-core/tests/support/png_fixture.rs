// test helper: an encoder failure here is a broken fixture, so it panics
#![allow(clippy::unwrap_used)]

use png::{BitDepth, ColorType, Encoder};

pub struct PngFixture;

impl PngFixture {
    pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

    pub fn encode(width: u32, height: u32, color: ColorType, depth: BitDepth, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, width, height);
        encoder.set_color(color);
        encoder.set_depth(depth);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(data).unwrap();
        writer.finish().unwrap();
        out
    }

    pub fn encode_indexed(
        width: u32,
        height: u32,
        depth: BitDepth,
        palette: &[u8],
        trns: &[u8],
        indices: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, width, height);
        encoder.set_color(ColorType::Indexed);
        encoder.set_depth(depth);
        encoder.set_palette(palette.to_vec());
        if !trns.is_empty() {
            encoder.set_trns(trns.to_vec());
        }
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(indices).unwrap();
        writer.finish().unwrap();
        out
    }

    // RGBA8 pixels that differ per position, alpha 255
    pub fn solid_rgba(width: u32, height: u32) -> Vec<u8> {
        (0..width * height).flat_map(|i| [i as u8, (i * 3) as u8, 200, 255]).collect()
    }

    // a two-frame RGBA8 animation; the default image is `first`
    pub fn encode_apng(width: u32, height: u32, first: &[u8], second: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = Encoder::new(&mut out, width, height);
        encoder.set_color(ColorType::Rgba);
        encoder.set_depth(BitDepth::Eight);
        encoder.set_animated(2, 0).unwrap();
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(first).unwrap();
        writer.write_image_data(second).unwrap();
        writer.finish().unwrap();
        out
    }

    // a signature, an IHDR with the given size, and an IEND; no pixel data
    pub fn header_only(width: u32, height: u32) -> Vec<u8> {
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        let mut out = Self::SIGNATURE.to_vec();
        out.extend(Self::chunk(b"IHDR", &ihdr));
        out.extend(Self::chunk(b"IEND", &[]));
        out
    }

    pub fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len() + 12);
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(data);
        let mut body = kind.to_vec();
        body.extend_from_slice(data);
        out.extend_from_slice(&Self::crc32(&body).to_be_bytes());
        out
    }

    // inserts a tEXt chunk after IHDR so the file is exactly `total` bytes long
    pub fn padded_to(png: &[u8], total: usize) -> Vec<u8> {
        let data_len = total - png.len() - 12;
        let mut text = b"k\0".to_vec();
        text.resize(data_len, b'x');
        let mut out = png[..33].to_vec();
        out.extend(Self::chunk(b"tEXt", &text));
        out.extend_from_slice(&png[33..]);
        out
    }

    // flips one byte inside the first IDAT payload
    pub fn with_idat_flipped(png: &[u8]) -> Vec<u8> {
        let mut out = png.to_vec();
        let mut at = 8;
        while at + 8 <= out.len() {
            let len = u32::from_be_bytes([out[at], out[at + 1], out[at + 2], out[at + 3]]) as usize;
            if &out[at + 4..at + 8] == b"IDAT" {
                out[at + 8 + len / 2] ^= 0xFF;
                return out;
            }
            at += 12 + len;
        }
        out
    }

    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xFFFF_FFFFu32;
        for &byte in bytes {
            crc ^= byte as u32;
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
}
