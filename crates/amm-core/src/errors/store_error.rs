use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StoreError {
    #[error("file operation failed: {}", path.display())]
    Io { path: PathBuf },
    #[error("the write queue is full")]
    QueueFull,
    #[error("the store is not available")]
    Unavailable,
    #[error("the pixel format {0} is not supported")]
    UnsupportedFormat(u32),
    #[error("the pixel buffer does not hold its width, height and row pitch")]
    BadImage,
}
