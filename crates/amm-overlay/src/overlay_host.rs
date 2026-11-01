use std::ptr::NonNull;

use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

/// What the overlay asks of the manager. Every call comes on the render thread, inside a Present of the bound
/// swap chain, except `game_ready`, which is also asked before a chain is bound.
///
/// # Safety
///
/// The frame `frame` returns is as `LayerRenderer::draw` requires: its layer pointer and every pointer of each
/// layer's mesh are null or point to their counts of live elements, until the next call of `frame`. A buffer
/// `capture_buffer` returns is writable for `height * row_pitch` bytes, with `row_pitch` at least `width * 4`, and
/// nothing else uses it until `capture_done` for that sequence, `textures_lost` or the next `frame`. A buffer
/// `stream_buffer` returns is as that, until `stream_done` for that id.
pub unsafe trait OverlayHost: Sync {
    /// True once the game has run its first tick. No swap chain is bound before it, so a launcher, splash or probe
    /// chain is never drawn on.
    fn game_ready(&self) -> bool;

    /// The layers to draw on this Present, or `None` when there are none to draw this time. The overlay fills the
    /// size, scale and clocks of `input`; its `menu_screen` is 0, for the host to set.
    fn frame(&self, input: &AmmHostFrameInput) -> Option<&AmmHostFrame>;

    /// The layer textures, the game picture and the captures in flight were dropped, after a resize or a new
    /// device. Called before the next `frame`, so the manager sends its textures again.
    fn textures_lost(&self);

    /// The buffer for the pixels of capture `sequence` and its row pitch in bytes, or `None` when the manager has
    /// none; the capture then fails.
    fn capture_buffer(&self, sequence: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)>;

    /// Capture `sequence` was written in full to its buffer in `format` (`ok` true), or could not be.
    fn capture_done(&self, sequence: u32, ok: bool, format: DXGI_FORMAT);

    /// The buffer for the pixels of capture stream frame `id` and its row pitch in bytes, or `None` when the
    /// manager has none; the frame then fails.
    fn stream_buffer(&self, id: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)>;

    /// Capture stream frame `id` was written in full to its buffer in `format` (`ok` true), or could not be.
    fn stream_done(&self, id: u32, ok: bool, format: DXGI_FORMAT);

    /// The size a `ResizeBuffers` of the bound chain takes in place of the asked `width` x `height`, or `None` to
    /// keep the asked size. Called on the render thread inside ResizeBuffers; must not block.
    fn render_size(&self, width: u32, height: u32) -> Option<(u32, u32)>;

    /// True while a render size is forced. Called on the render thread for each Present of the bound chain; must
    /// not block.
    fn size_forced(&self) -> bool;

    /// A Present of the bound chain ran no frame: no renderer, no target, or `frame` gave `None`. The host puts the
    /// cursor and the vanilla HUD back to the game if a frame had taken them.
    fn frame_skipped(&self);
}
