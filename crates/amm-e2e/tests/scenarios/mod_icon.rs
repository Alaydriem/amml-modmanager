use amm_core::structs::registry::ModIndex;

use crate::support::{Descriptors, HostFixture, SimMod};

struct IconPng;

impl IconPng {
    fn solid(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let pixels: Vec<u8> = rgba.iter().copied().cycle().take((width * height * 4) as usize).collect();
        let mut writer = encoder.write_header().unwrap_or_else(|error| panic!("png header: {error}"));
        writer.write_image_data(&pixels).unwrap_or_else(|error| panic!("png data: {error}"));
        writer.finish().unwrap_or_else(|error| panic!("png finish: {error}"));
        out
    }

    fn generation(fixture: &HostFixture, index: ModIndex) -> Option<u64> {
        fixture.with_runtime(|runtime| runtime.manager().icon(index).map(|entry| entry.generation))
    }
}

#[test]
fn icon_set_replaced_cleared_and_freed() {
    let fixture = HostFixture::new();
    let index = ModIndex(0);
    let png_a = IconPng::solid(16, 16, [200, 30, 30, 255]);
    let png_b = IconPng::solid(32, 24, [30, 30, 200, 255]);
    let mod_a = SimMod::new(&fixture, Descriptors::with_claims("iconic", &[], ""), "");

    let first = mod_a.link().set_icon(&png_a);
    let g1 = IconPng::generation(&fixture, index);
    assert!(first.is_ok());
    assert!(g1.is_some());

    let second = mod_a.link().set_icon(&png_b);
    let g2 = IconPng::generation(&fixture, index);
    assert!(second.is_ok());
    assert!(g2 > g1);

    let bad = mod_a.link().set_icon(b"not a png");
    assert!(bad.is_err());
    assert_eq!(IconPng::generation(&fixture, index), g2);

    let cleared = mod_a.link().set_icon(&[]);
    assert!(cleared.is_ok());
    assert_eq!(IconPng::generation(&fixture, index), None);

    let again = mod_a.link().set_icon(&png_a);
    assert!(again.is_ok());
    assert!(IconPng::generation(&fixture, index).is_some());

    drop(mod_a);
    assert_eq!(IconPng::generation(&fixture, index), None);
}
