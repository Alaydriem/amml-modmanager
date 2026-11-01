/// A decoded mod icon: straight (not premultiplied) RGBA8, row-major.
#[derive(Debug, Clone, PartialEq)]
pub struct ModIcon {
    pub width: u16,
    pub height: u16,
    // length is width * height * 4
    pub rgba: Vec<u8>,
}

impl ModIcon {
    /// The size of the pixel data in bytes.
    pub fn bytes(&self) -> usize {
        self.rgba.len()
    }
}
