#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IconError {
    #[error("the icon is {len} bytes, over the byte cap")]
    TooLarge { len: usize },
    #[error("the icon is not a PNG file")]
    NotPng,
    #[error("the icon is {width} by {height} pixels, outside the allowed size")]
    BadSize { width: u32, height: u32 },
    #[error("the icon data is corrupt")]
    Corrupt,
}
