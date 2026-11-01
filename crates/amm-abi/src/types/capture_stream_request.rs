use amm_core::traits::AbiSized;

/// One frame asked of the capture stream, the argument of `capture_stream_request`. The manager does not read
/// `tag`; it comes back with the frame. `size` is `size_of` the struct the mod was built with. `present_seq` 0 asks
/// for the next Present; any other number is the one Present the frame is taken from, as counted by the
/// `present_count` slot, and the frame fails `Missed` when that Present passed first.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmCaptureStreamRequest {
    pub size: u32,
    pub tag: u64,
    pub present_seq: u64,
}

impl AbiSized for AmmCaptureStreamRequest {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        Self { size: size_of::<Self>() as u32, tag: 0, present_seq: 0 }
    }
}
