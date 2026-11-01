/// A back-buffer size in physical pixels. 0 x 0 stands for no size where a field says so.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmRenderSize {
    pub width: u32,
    pub height: u32,
}
