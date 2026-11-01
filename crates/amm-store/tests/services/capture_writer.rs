use std::fs::File;
use std::path::{Path, PathBuf};

use amm_core::errors::StoreError;
use amm_core::structs::capture::{CaptureImage, PixelFormat};
use amm_store::services::CaptureWriter;

fn image(path: PathBuf, format: u32) -> CaptureImage {
    let pixels = vec![30, 20, 10, 255, 70, 60, 50, 255, 1, 2, 3, 4, 5, 6, 7, 8];
    CaptureImage { sequence: 4, path, pixels, width: 2, height: 2, row_pitch: 8, format }
}

fn temp_of(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

#[test]
fn a_capture_is_written_as_an_opaque_eight_bit_rgb_png() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");

    CaptureWriter::write(&image(path.clone(), PixelFormat::DXGI_B8G8R8A8_UNORM)).unwrap();

    let decoder = png::Decoder::new(std::io::BufReader::new(File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut out = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut out).unwrap();
    assert_eq!((info.width, info.height), (2, 2));
    assert_eq!(info.color_type, png::ColorType::Rgb);
    assert_eq!(info.bit_depth, png::BitDepth::Eight);
    assert_eq!(reader.info().srgb, Some(png::SrgbRenderingIntent::Perceptual));
    assert_eq!(&out[..info.buffer_size()], &[10, 20, 30, 50, 60, 70, 3, 2, 1, 7, 6, 5]);
    assert!(!temp_of(&path).exists());
}

#[test]
fn a_capture_replaces_an_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("SHOT.PNG");
    std::fs::write(&path, b"old").unwrap();

    CaptureWriter::write(&image(path.clone(), PixelFormat::DXGI_R8G8B8A8_UNORM)).unwrap();

    assert_eq!(&std::fs::read(&path).unwrap()[..4], b"\x89PNG");
}

#[test]
fn a_refused_format_writes_no_file_and_no_temporary_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");

    let result = CaptureWriter::write(&image(path.clone(), 10));

    assert_eq!(result, Err(StoreError::UnsupportedFormat(10)));
    assert!(!path.exists());
    assert!(!temp_of(&path).exists());
}

#[test]
fn a_failed_rename_removes_the_temporary_file_and_leaves_the_target_as_it_was() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("shot.png");
    std::fs::create_dir(&path).unwrap();

    let result = CaptureWriter::write(&image(path.clone(), PixelFormat::DXGI_R8G8B8A8_UNORM));

    assert!(matches!(result, Err(StoreError::Io { .. })));
    assert!(path.is_dir());
    assert!(!temp_of(&path).exists());
}

#[test]
fn a_missing_folder_writes_nothing_and_makes_no_folder() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gone").join("shot.png");

    let result = CaptureWriter::write(&image(path.clone(), PixelFormat::DXGI_R8G8B8A8_UNORM));

    assert!(matches!(result, Err(StoreError::Io { .. })));
    assert!(!dir.path().join("gone").exists());
}

// Decodes the PNG at `path` into its RGB bytes.
fn rgb_of(path: &Path) -> Vec<u8> {
    let Ok(file) = File::open(path) else { return Vec::new() };
    let Ok(mut reader) = png::Decoder::new(std::io::BufReader::new(file)).read_info() else { return Vec::new() };
    let mut out = vec![0; reader.output_buffer_size().unwrap_or(0)];
    match reader.next_frame(&mut out) {
        Ok(info) => out[..info.buffer_size()].to_vec(),
        Err(_) => Vec::new(),
    }
}

#[test]
fn the_srgb_and_ten_bit_formats_are_written_through_the_writer() {
    let dir = tempfile::tempdir().unwrap();
    let ten_bit = |r: u32, g: u32, b: u32| (r | (g << 10) | (b << 20)).to_le_bytes();
    let mut packed = ten_bit(1023, 0, 512).to_vec();
    packed.extend_from_slice(&ten_bit(0, 1023, 0));
    packed.extend_from_slice(&ten_bit(0, 0, 1023));
    packed.extend_from_slice(&ten_bit(512, 512, 512));
    let cases = [
        (
            PixelFormat::DXGI_R8G8B8A8_UNORM_SRGB,
            image(dir.path().join("rgba.png"), 0).pixels,
            vec![30, 20, 10, 70, 60, 50, 1, 2, 3, 5, 6, 7],
        ),
        (
            PixelFormat::DXGI_B8G8R8A8_UNORM_SRGB,
            image(dir.path().join("bgra.png"), 0).pixels,
            vec![10, 20, 30, 50, 60, 70, 3, 2, 1, 7, 6, 5],
        ),
        (PixelFormat::DXGI_R10G10B10A2_UNORM, packed, vec![255, 0, 128, 0, 255, 0, 0, 0, 255, 128, 128, 128]),
    ];

    for (index, (format, pixels, expected)) in cases.into_iter().enumerate() {
        let path = dir.path().join(format!("shot{index}.png"));
        let shot = CaptureImage { pixels, format, ..image(path.clone(), format) };

        CaptureWriter::write(&shot).unwrap();

        assert_eq!(rgb_of(&path), expected, "format {format}");
    }
}
