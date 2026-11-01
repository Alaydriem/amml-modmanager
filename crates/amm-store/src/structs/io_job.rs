use amm_core::structs::capture::CaptureImage;
use amm_core::structs::write::WriteRequest;

/// One job of the I/O thread: a config file to write, or a capture to encode and write as a PNG.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IoJob {
    Config(WriteRequest),
    Capture(CaptureImage),
}
