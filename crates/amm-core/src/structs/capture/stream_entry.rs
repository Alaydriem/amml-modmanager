use crate::structs::capture::{CaptureStreamFrame, CaptureStreamStage};
use crate::structs::registry::ModIndex;
use crate::structs::render::RenderSize;

/// One slot of the capture stream ring. `id` names the copy to the overlay; `order` is the request order of the
/// frame, which is the order the mod gets it back in; `size` is the back buffer size of the Present that offered it.
/// `frame.pixels` keeps its buffer while the slot is free, so a frame of the same size does not allocate, until the
/// stream is closed. `owner` is `None` for a free slot.
#[derive(Debug, Default)]
pub struct CaptureStreamEntry {
    pub id: u32,
    pub owner: Option<ModIndex>,
    pub order: u64,
    // the Present the frame must come from, 0 for the next one
    pub target: u64,
    pub size: RenderSize,
    pub stage: CaptureStreamStage,
    pub frame: CaptureStreamFrame,
}

impl CaptureStreamEntry {
    /// The slot is free again; its buffer is kept.
    pub fn free(&mut self) {
        self.id = 0;
        self.owner = None;
        self.stage = CaptureStreamStage::Free;
    }
}
