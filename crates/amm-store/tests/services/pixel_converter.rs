use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, PixelFormat};
use amm_store::services::PixelConverter;

fn image(pixels: Vec<u8>, width: u32, height: u32, row_pitch: u32, format: u32) -> CaptureImage {
    CaptureImage { sequence: 1, path: "shot.png".into(), pixels, width, height, row_pitch, format }
}

// Two pixels in one row, then two padding bytes the converter must skip.
fn two_pixels(first: [u8; 4], second: [u8; 4]) -> Vec<u8> {
    let mut row = first.to_vec();
    row.extend_from_slice(&second);
    row.extend_from_slice(&[0xEE, 0xEE]);
    row
}

#[test]
fn rgba_buffers_are_copied_as_is_with_alpha_and_row_padding_dropped() {
    for format in [PixelFormat::DXGI_R8G8B8A8_UNORM, PixelFormat::DXGI_R8G8B8A8_UNORM_SRGB] {
        let pixels = two_pixels([10, 20, 30, 40], [50, 60, 70, 0]);

        let rgb = PixelConverter::to_rgb(&image(pixels, 2, 1, 10, format)).unwrap();

        assert_eq!(rgb, vec![10, 20, 30, 50, 60, 70], "format {format}");
    }
}

#[test]
fn bgra_buffers_are_swizzled_to_rgb() {
    for format in [PixelFormat::DXGI_B8G8R8A8_UNORM, PixelFormat::DXGI_B8G8R8A8_UNORM_SRGB] {
        let pixels = two_pixels([30, 20, 10, 255], [70, 60, 50, 255]);

        let rgb = PixelConverter::to_rgb(&image(pixels, 2, 1, 10, format)).unwrap();

        assert_eq!(rgb, vec![10, 20, 30, 50, 60, 70], "format {format}");
    }
}

#[test]
fn ten_bit_buffers_are_scaled_to_eight_bits() {
    let word = |r: u32, g: u32, b: u32| (r | (g << 10) | (b << 20) | (3 << 30)).to_le_bytes();
    let pixels = two_pixels(word(1023, 0, 512), word(4, 1019, 2));

    let rgb = PixelConverter::to_rgb(&image(pixels, 2, 1, 10, PixelFormat::DXGI_R10G10B10A2_UNORM)).unwrap();

    assert_eq!(rgb, vec![255, 0, 128, 1, 254, 0]);
}

#[test]
fn rows_are_read_at_their_pitch() {
    let mut pixels = vec![1, 2, 3, 4, 0, 0, 0, 0];
    pixels.extend_from_slice(&[5, 6, 7, 8]);

    let rgb = PixelConverter::to_rgb(&image(pixels, 1, 2, 8, PixelFormat::DXGI_R8G8B8A8_UNORM)).unwrap();

    assert_eq!(rgb, vec![1, 2, 3, 5, 6, 7]);
}

#[test]
fn an_hdr_or_unknown_format_is_refused() {
    let pixels = vec![0; 8 * 4];

    let float16 = PixelConverter::to_rgb(&image(pixels.clone(), 1, 4, 8, 10));
    let unknown = PixelConverter::to_rgb(&image(pixels, 1, 4, 8, 0));

    assert_eq!(float16, Err(StoreError::UnsupportedFormat(10)));
    assert_eq!(unknown, Err(StoreError::UnsupportedFormat(0)));
}

#[test]
fn a_buffer_shorter_than_its_rows_or_a_pitch_below_the_width_is_refused() {
    let short = PixelConverter::to_rgb(&image(vec![0; 7], 2, 1, 8, PixelFormat::DXGI_R8G8B8A8_UNORM));
    let narrow = PixelConverter::to_rgb(&image(vec![0; 16], 2, 2, 4, PixelFormat::DXGI_R8G8B8A8_UNORM));
    let empty = PixelConverter::to_rgb(&image(Vec::new(), 0, 0, 0, PixelFormat::DXGI_R8G8B8A8_UNORM));

    assert_eq!(short, Err(StoreError::BadImage));
    assert_eq!(narrow, Err(StoreError::BadImage));
    assert_eq!(empty, Err(StoreError::BadImage));
}
