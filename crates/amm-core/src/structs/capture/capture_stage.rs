/// Where one capture is. `Pending` until the overlay takes it at a Present, `Taken` while the overlay copies the back
/// buffer, `Writing` while the I/O thread encodes and writes the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureStage {
    Pending,
    Taken,
    Writing,
}
