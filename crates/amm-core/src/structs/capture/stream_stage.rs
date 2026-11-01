use crate::structs::capture::CaptureStreamFault;

/// Where one slot of the capture stream ring is. `Pending` until a Present offers it to the overlay, `Offered` while
/// the overlay copies it (`age` Presents since the offer; `asked` once the overlay took its buffer), then `Ready` or
/// `Failed` until the mod takes it. `Lost` is a copy the device dropped; the next Present fails it `Resized` when the
/// back buffer size changed since its offer, else `DeviceLost`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum CaptureStreamStage {
    #[default]
    Free,
    Pending,
    Offered {
        age: u32,
        asked: bool,
    },
    Lost,
    Ready,
    Failed(CaptureStreamFault),
}
