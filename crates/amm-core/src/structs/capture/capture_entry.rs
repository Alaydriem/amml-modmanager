use std::path::PathBuf;

use crate::structs::capture::{CaptureStage, ScreenshotLayers};
use crate::structs::registry::ModIndex;

/// One capture in flight. `owner` is `None` once the mod that asked for it has unregistered: the capture then
/// keeps its place until the overlay or the I/O thread ends it and sends no event. A capture still `Taken` then
/// writes no file; one already `Writing` was queued with its path and still writes its file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureEntry {
    pub sequence: u32,
    pub owner: Option<ModIndex>,
    pub path: PathBuf,
    pub layers: ScreenshotLayers,
    pub stage: CaptureStage,
    // manager ticks spent `Pending`
    pub pending_ticks: u32,
}

impl CaptureEntry {
    pub fn new(sequence: u32, owner: ModIndex, path: PathBuf, layers: ScreenshotLayers) -> Self {
        Self { sequence, owner: Some(owner), path, layers, stage: CaptureStage::Pending, pending_ticks: 0 }
    }
}
