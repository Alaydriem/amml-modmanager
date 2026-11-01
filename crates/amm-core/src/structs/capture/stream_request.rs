/// One frame a mod asks the capture stream for: a Present copies the back buffer for it, game-only, and the frame
/// comes back with `tag`. The manager does not read the tag; a mod uses it to name its frame, for example the frame
/// number of a video. `present_seq` 0 asks for the next Present; any other number names the one Present the frame
/// is taken from, and the frame fails `Missed` when that Present passed before the manager saw the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureStreamRequest {
    pub tag: u64,
    pub present_seq: u64,
}

impl CaptureStreamRequest {
    pub fn new(tag: u64) -> Self {
        Self { tag, present_seq: 0 }
    }

    /// A request for the Present numbered `present_seq` only.
    pub fn at(tag: u64, present_seq: u64) -> Self {
        Self { tag, present_seq }
    }
}
