/// How the write of one capture ended: `ok` when its PNG file is in place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureOutcome {
    pub sequence: u32,
    pub ok: bool,
}
