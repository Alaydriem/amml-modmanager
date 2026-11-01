#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CaptureError {
    #[error("no mod is registered in that slot")]
    UnknownMod,
    #[error("a capture needs an active Hud or Overlay lease")]
    NotPermitted,
    #[error("the screenshot path is refused")]
    InvalidPath,
    #[error("a capture of this mod is in flight, or all capture slots are in use")]
    Busy,
}
