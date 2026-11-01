/// Why one frame of the capture stream has no pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStreamFault {
    /// The overlay could not copy, map or write the back buffer.
    CopyFailed,
    /// The back buffer is in a DXGI format the manager does not read (`PixelFormat::from_dxgi`), for example an
    /// HDR float format.
    UnsupportedFormat { format: u32 },
    /// The copy was not read within `CaptureStreamBook::MAX_FRAMES` Presents.
    TimedOut,
    /// The device dropped its resources while the copy was in flight, with no change of the back buffer size.
    DeviceLost,
    /// The back buffer changed size while the copy was in flight (a window resize, or a render size forced or
    /// ended). The frames asked for after the change come at the new size.
    Resized,
    /// The mod lost its `Hud` and `Overlay` leases before the frame was taken.
    NotPermitted,
    /// The request named a Present that had passed when the manager saw it.
    Missed,
}
