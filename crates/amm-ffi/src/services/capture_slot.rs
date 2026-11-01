/// One capture the overlay took at a Present, from the frame that offered it until the manager hands its pixels on
/// or ends it. `pixels` has no capacity until the overlay asks for a buffer; it then reserves `height` rows of
/// `row_pitch` bytes once and never grows. Its length stays 0 until the overlay reports a good copy, and is then the
/// whole buffer.
#[derive(Debug)]
pub struct CaptureSlot {
    pub sequence: u32,
    // Presents since the capture was offered
    pub age: u32,
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    // the overlay's report: whether the copy worked, and the DXGI format of the pixels
    pub report: Option<(bool, u32)>,
}

impl CaptureSlot {
    pub fn new(sequence: u32) -> Self {
        Self { sequence, age: 0, pixels: Vec::new(), width: 0, height: 0, row_pitch: 0, report: None }
    }

    /// True once the overlay asked for a buffer and it was reserved.
    pub fn has_buffer(&self) -> bool {
        self.pixels.capacity() != 0
    }

    /// The bytes of the whole buffer, `height` rows of `row_pitch` bytes.
    pub fn buffer_len(&self) -> usize {
        self.row_pitch as usize * self.height as usize
    }
}
