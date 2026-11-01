use amm_core::structs::icon::ModIcon;

// A solid-colour icon for tests
pub struct IconFixture;

impl IconFixture {
    pub fn solid(width: u16, height: u16, rgba: [u8; 4]) -> ModIcon {
        let pixels = usize::from(width) * usize::from(height);
        ModIcon { width, height, rgba: rgba.repeat(pixels) }
    }
}
