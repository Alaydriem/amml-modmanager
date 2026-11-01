use crate::structs::registry::ModIndex;

/// One frame of the capture stream whose pixel buffer the manager lent to a mod. `pixels` is as in
/// `CaptureStreamFrame`. The buffer is the mod's until it gives `lend_id` back; the book never writes it. `owner` is
/// the mod that holds the lease, and `orphaned` is set when that mod ended its stream, so that the buffer is freed,
/// not kept for a later frame, once it is given back.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LentStreamFrame {
    pub tag: u64,
    pub width: u32,
    pub height: u32,
    pub row_pitch: u32,
    pub format: u32,
    pub pixels: Vec<u8>,
    pub present_seq: u64,
    pub lend_id: u64,
    pub owner: Option<ModIndex>,
    pub orphaned: bool,
}
