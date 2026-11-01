/// Why a capture stream frame has no pixels, as the `fault` byte of an `AmmCaptureStreamFrame`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmCaptureStreamFault {
    /// The frame holds its pixels.
    None = 0,
    /// The back buffer could not be copied, read or written.
    CopyFailed = 1,
    /// The back buffer's DXGI format, in `format`, is not one the manager reads.
    UnsupportedFormat = 2,
    /// The copy was not read within 30 Presents.
    TimedOut = 3,
    /// The device dropped its resources while the copy was in flight, with no change of the back buffer size.
    DeviceLost = 4,
    /// The mod lost its `Hud` and `Overlay` leases before a Present took the frame.
    NotPermitted = 5,
    /// The back buffer changed size while the copy was in flight; frames asked for after it come at the new size.
    Resized = 6,
    /// The request named a Present that had passed when the manager saw it.
    Missed = 7,
}
