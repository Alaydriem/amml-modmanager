use crate::structs::capture::{CaptureRequest, ScreenshotLayers};
use crate::structs::registry::ModIndex;

/// A fixed list of at most `CAPACITY` captures, game-only and with mod layers in one list, so both share the
/// count. `of` gives the two sets apart. It is `Copy` and holds no heap memory, so the frame path copies
/// it without allocating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PendingCaptures {
    entries: [CaptureRequest; Self::CAPACITY],
    len: usize,
}

impl PendingCaptures {
    /// The most captures in flight over all mods, one per overlay staging texture.
    pub const CAPACITY: usize = 4;

    pub fn new() -> Self {
        Self { entries: [CaptureRequest::new(0, ModIndex(0), ScreenshotLayers::GameOnly); Self::CAPACITY], len: 0 }
    }

    /// Adds a capture. Returns false and changes nothing when the list is full.
    pub fn push(&mut self, request: CaptureRequest) -> bool {
        let Some(slot) = self.entries.get_mut(self.len) else {
            return false;
        };
        *slot = request;
        self.len += 1;
        true
    }

    pub fn as_slice(&self) -> &[CaptureRequest] {
        &self.entries[..self.len]
    }

    /// The captures taken at the capture point `layers`, in list order. It does not allocate.
    pub fn of(&self, layers: ScreenshotLayers) -> impl Iterator<Item = &CaptureRequest> {
        self.as_slice().iter().filter(move |request| request.layers == layers)
    }
}

impl Default for PendingCaptures {
    fn default() -> Self {
        Self::new()
    }
}
