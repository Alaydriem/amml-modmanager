/// Which request a `FrameCapture` pool serves, named in its log lines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureKind {
    Screenshot,
    Stream,
}

impl CaptureKind {
    /// The kind as a log field.
    pub fn name(self) -> &'static str {
        match self {
            Self::Screenshot => "screenshot",
            Self::Stream => "stream",
        }
    }
}
