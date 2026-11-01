use amm_core::errors::IconError;
use amm_core::services::IconDecoder;
use png::{BitDepth, ColorType};

use crate::support::png_fixture::PngFixture;

#[test]
fn rgba_png_decodes_to_its_size_and_pixels() {
    let pixels: Vec<u8> = (0..24).map(|i| i * 10).collect();
    let png = PngFixture::encode(3, 2, ColorType::Rgba, BitDepth::Eight, &pixels);
    let icon = IconDecoder::decode(&png).unwrap();
    assert_eq!((icon.width, icon.height), (3, 2));
    assert_eq!(icon.rgba, pixels);
    assert_eq!(icon.bytes(), 24);
}

#[test]
fn rgb_grey_palette_and_16_bit_inputs_become_rgba8() {
    let rgb = PngFixture::encode(2, 1, ColorType::Rgb, BitDepth::Eight, &[1, 2, 3, 4, 5, 6]);
    let icon = IconDecoder::decode(&rgb).unwrap();
    assert_eq!(icon.rgba, vec![1, 2, 3, 255, 4, 5, 6, 255]);

    let grey = PngFixture::encode(2, 1, ColorType::Grayscale, BitDepth::Eight, &[7, 9]);
    let icon = IconDecoder::decode(&grey).unwrap();
    assert_eq!(icon.rgba, vec![7, 7, 7, 255, 9, 9, 9, 255]);

    let grey_alpha = PngFixture::encode(2, 1, ColorType::GrayscaleAlpha, BitDepth::Eight, &[7, 100, 9, 200]);
    let icon = IconDecoder::decode(&grey_alpha).unwrap();
    assert_eq!(icon.rgba, vec![7, 7, 7, 100, 9, 9, 9, 200]);

    let indexed = PngFixture::encode_indexed(2, 1, BitDepth::Eight, &[10, 20, 30, 40, 50, 60], &[128], &[0, 1]);
    let icon = IconDecoder::decode(&indexed).unwrap();
    assert_eq!(icon.rgba, vec![10, 20, 30, 128, 40, 50, 60, 255]);

    let wide: Vec<u8> = [0x1234u16, 0x5678, 0x9ABC, 0xDEF0].iter().flat_map(|v| v.to_be_bytes()).collect();
    let rgba16 = PngFixture::encode(1, 1, ColorType::Rgba, BitDepth::Sixteen, &wide);
    let icon = IconDecoder::decode(&rgba16).unwrap();
    assert_eq!(icon.rgba, vec![0x12, 0x56, 0x9A, 0xDE]);
}

#[test]
fn input_over_the_byte_cap_is_refused_before_decoding() {
    let bytes = vec![0u8; IconDecoder::MAX_BYTES + 1];
    assert_eq!(IconDecoder::decode(&bytes), Err(IconError::TooLarge { len: IconDecoder::MAX_BYTES + 1 }));
}

#[test]
fn exactly_max_bytes_is_not_refused_for_size() {
    let png = PngFixture::encode(2, 2, ColorType::Rgba, BitDepth::Eight, &PngFixture::solid_rgba(2, 2));
    let padded = PngFixture::padded_to(&png, IconDecoder::MAX_BYTES);
    assert_eq!(padded.len(), IconDecoder::MAX_BYTES);
    assert!(IconDecoder::decode(&padded).is_ok());
}

#[test]
fn header_claiming_more_than_256_px_is_refused() {
    for (width, height) in [(65535, 65535), (257, 1), (1, 257)] {
        let png = PngFixture::header_only(width, height);
        assert_eq!(IconDecoder::decode(&png), Err(IconError::BadSize { width, height }));
    }
}

#[test]
fn a_256_square_decodes() {
    let png = PngFixture::encode(256, 256, ColorType::Rgba, BitDepth::Eight, &PngFixture::solid_rgba(256, 256));
    let icon = IconDecoder::decode(&png).unwrap();
    assert_eq!((icon.width, icon.height), (256, 256));
    assert_eq!(icon.bytes(), 256 * 256 * 4);
}

#[test]
fn bad_signature_is_not_png() {
    assert_eq!(IconDecoder::decode(b"GIF89a......"), Err(IconError::NotPng));
    assert_eq!(IconDecoder::decode(&[]), Err(IconError::NotPng));
}

#[test]
fn truncated_or_corrupt_data_is_corrupt() {
    let png = PngFixture::encode(8, 8, ColorType::Rgba, BitDepth::Eight, &PngFixture::solid_rgba(8, 8));
    let cut = &png[..png.len() / 2];
    assert_eq!(IconDecoder::decode(cut), Err(IconError::Corrupt));

    let flipped = PngFixture::with_idat_flipped(&png);
    assert_eq!(IconDecoder::decode(&flipped), Err(IconError::Corrupt));
}

#[test]
fn apng_decodes_its_default_image() {
    let first = PngFixture::solid_rgba(2, 2);
    let second: Vec<u8> = first.iter().map(|b| b.wrapping_add(1)).collect();
    let png = PngFixture::encode_apng(2, 2, &first, &second);
    let icon = IconDecoder::decode(&png).unwrap();
    assert_eq!(icon.rgba, first);
}

#[test]
fn short_tail_or_missing_ihdr_is_not_png() {
    let mut short = PngFixture::SIGNATURE.to_vec();
    short.extend_from_slice(&[0, 0, 0, 13]);
    assert_eq!(IconDecoder::decode(&short), Err(IconError::NotPng));

    let mut no_ihdr = PngFixture::SIGNATURE.to_vec();
    no_ihdr.extend(PngFixture::chunk(b"IEND", &[]));
    assert_eq!(IconDecoder::decode(&no_ihdr), Err(IconError::NotPng));
}

#[test]
fn low_bit_depth_grey_and_palette_become_rgba8() {
    // 1-bit grey, 8 pixels in one row: 1,0,1,0,1,0,1,0
    let grey = PngFixture::encode(8, 1, ColorType::Grayscale, BitDepth::One, &[0b1010_1010]);
    let icon = IconDecoder::decode(&grey).unwrap();
    assert_eq!(icon.rgba.len(), 8 * 4);
    assert_eq!(&icon.rgba[0..4], &[255, 255, 255, 255]);
    assert_eq!(&icon.rgba[4..8], &[0, 0, 0, 255]);

    // 4-bit palette, 2 pixels: index 1 then 0
    let indexed = PngFixture::encode_indexed(2, 1, BitDepth::Four, &[1, 2, 3, 4, 5, 6], &[], &[0x10]);
    let icon = IconDecoder::decode(&indexed).unwrap();
    assert_eq!(icon.rgba, vec![4, 5, 6, 255, 1, 2, 3, 255]);
}
