use crate::structs::capture::CaptureStreamFault;

/// The frame with `tag` of the capture stream failed with `fault`. It takes the frame's place in the stream order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureStreamFailure {
    pub tag: u64,
    pub fault: CaptureStreamFault,
}

impl CaptureStreamFailure {
    pub fn new(tag: u64, fault: CaptureStreamFault) -> Self {
        Self { tag, fault }
    }
}
