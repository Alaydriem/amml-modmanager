use super::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmCallbacks, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus};

/// The registry table `alaydriem.modmanager.layers`: the mod's egui layers, its texture budget, the cursor mode, the
/// game picture and the render size. Game thread unless a function says otherwise. New functions are only added at
/// the end, with a higher minor. A null slot means the function is absent. After the manager is disabled every
/// function answers `Disabled`, or nothing for `set_cursor_mode`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmLayersTable {
    pub size: u32,
    pub version: u32,
    /// Sets the mod's frame and `textures_lost` callbacks, which run on the render thread, replacing the ones it
    /// gave before. `callbacks` is copied during the call, up to the `size` it declares; a `size` below the 3.0
    /// `AmmCallbacks` is `InvalidArgument`, a null `callbacks` is `NullPointer`. A call from inside one of the mod's
    /// callbacks is `NotPermitted`.
    pub set_callbacks: Option<unsafe extern "C" fn(handle: AmmHandle, callbacks: *const AmmCallbacks) -> AmmStatus>,
    /// The texture budget the manager gave the mod: at most `out_bytes` bytes of textures in at most `out_count`
    /// textures. Both are written on `Ok`; a null out pointer is `NullPointer`, and nothing is written.
    pub texture_budget:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus>,
    /// Turns the cursor mode on (1) or off (0). Any other value is ignored. Turning it on needs an active `Overlay`
    /// lease.
    pub set_cursor_mode: Option<unsafe extern "C" fn(handle: AmmHandle, on: u8)>,
    /// Whether the mod may draw the live game picture, and its size: `Ok` while the mod holds `Overlay`, with the
    /// back buffer size of the last Present in physical pixels (both 0 before the first Present). `NotPermitted`
    /// while it does not hold `Overlay`, and nothing is written. A null out pointer is `NullPointer`; a call from
    /// inside a frame callback is `NotPermitted` (the host is busy composing the frame).
    pub game_picture:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_width: *mut u32, out_height: *mut u32) -> AmmStatus>,
    /// Sets where the mod draws the game picture, `[min_x, min_y, max_x, max_y]` in back-buffer pixels, or clears it
    /// with a null `rect`. While the mod holds `Overlay`, a pointer inside it reaches the game at the matching place
    /// of the full window. A rect with a NaN or no area is `InvalidArgument`. The manager reads `rect` only during
    /// the call.
    pub set_game_viewport: Option<unsafe extern "C" fn(handle: AmmHandle, rect: *const [f32; 4]) -> AmmStatus>,
    /// Makes the game draw at `size`, in physical pixels, whatever the size of its window, or ends the size the mod
    /// forces with a null `size`. The mod must hold `RenderSize`, else `NotPermitted`. Each side is 1 to 16384, else
    /// `InvalidArgument`. The size ends on a null `size`, when the mod no longer holds `RenderSize` and when the
    /// manager stops. The manager reads `size` only during the call.
    pub set_render_size: Option<unsafe extern "C" fn(handle: AmmHandle, size: *const AmmRenderSize) -> AmmStatus>,
    /// Writes to `out_state` the size the mod forces (0 x 0 for none) and the back buffer size and DXGI format of the
    /// last Present. `NotPermitted` while the mod does not hold `RenderSize`, and nothing is written. A null
    /// `out_state` is `NullPointer`. The mod sets `out_state.size` to the size of its struct; the manager writes only
    /// that many bytes and sets `size` to the bytes it wrote. A `size` below the 3.0 struct is `InvalidArgument`.
    pub render_size: Option<unsafe extern "C" fn(handle: AmmHandle, out_state: *mut AmmRenderSizeState) -> AmmStatus>,
}

// SAFETY: `AmmLayersTable` is `#[repr(C)]` and starts with `size: u32, version: u32`, and `size` is the byte size of
// the provider's table.
#[allow(unsafe_code)]
unsafe impl amml_api::Provided for AmmLayersTable {
    const NAME: &'static str = "alaydriem.modmanager.layers";
    const MAJOR: u16 = AMM_ABI_MAJOR as u16;
    const MINOR: u16 = AMM_ABI_MINOR as u16;
}
