use amm_core::traits::AbiSized;

use super::AmmRenderSize;

/// What `render_size` reads back: `wanted` is the size the caller forces, 0 x 0 when it forces none; `actual` is
/// the back buffer of the last Present in physical pixels (0 x 0 before the first) and `format` its DXGI format
/// (0 before the first). A forced size is in effect once `actual` equals `wanted`. The mod sets `size` to `size_of`
/// its struct; the manager writes only that many bytes and sets `size` to the bytes it wrote.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmRenderSizeState {
    pub size: u32,
    pub wanted: AmmRenderSize,
    pub actual: AmmRenderSize,
    pub format: u32,
}

impl AbiSized for AmmRenderSizeState {
    const BASE_SIZE: usize = size_of::<Self>();

    fn blank() -> Self {
        let none = AmmRenderSize { width: 0, height: 0 };
        Self { size: size_of::<Self>() as u32, wanted: none, actual: none, format: 0 }
    }
}
