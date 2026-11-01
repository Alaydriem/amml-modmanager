use amm_abi::types::AmmCursorShape;
use amm_core::services::capture::CaptureStreamBook;
use amm_core::structs::capture::PendingCaptures;
use amm_core::structs::mesh::GamePicture;
use amm_core::structs::render::RenderSize;

use super::AmmHostLayer;

/// The most captures one `AmmHostFrame` asks for, one per overlay staging texture.
pub const AMM_HOST_MAX_CAPTURES: usize = 4;

const _: () = assert!(AMM_HOST_MAX_CAPTURES == PendingCaptures::CAPACITY);

/// The most capture stream copies one `AmmHostFrame` asks for, one per overlay stream staging texture.
pub const AMM_HOST_MAX_STREAM_FRAMES: usize = 4;

const _: () = assert!(AMM_HOST_MAX_STREAM_FRAMES == CaptureStreamBook::RING);

/// The Presents a capture may take from the frame that offers it to `amm_host_capture_done`. The manager fails a
/// capture not reported by then, and the overlay stops waiting on its staging copy after as many Presents.
pub const AMM_HOST_CAPTURE_MAX_FRAMES: u32 = 30;

const _: () = assert!(AMM_HOST_CAPTURE_MAX_FRAMES == CaptureStreamBook::MAX_FRAMES);

/// The texture id of a draw command that shows the game picture (`GamePicture::HOST_TEXTURE`). The overlay draws its
/// own copy of the back buffer for it; it is never in the texture registry.
pub const AMM_HOST_GAME_PICTURE_TEXTURE: u64 = 0x8000_0000_FFFF_FFFF;

const _: () = assert!(AMM_HOST_GAME_PICTURE_TEXTURE == GamePicture::HOST_TEXTURE);

/// What the overlay draws for one Present: `layer_count` layers at `layers`, bottom to top, valid until the next
/// `amm_host_frame`. `freed_owners` has bit N set when mod slot N left since the last frame, so the overlay drops
/// every texture of that owner. `cursor_free` releases the mouse grab and shows `cursor`; `hide_vanilla_hud`
/// hides the game HUD.
///
/// Two sets of screenshots are taken at this Present, at two capture points. The first `capture_count` entries of
/// `captures` are the game-only ones: the overlay copies the back buffer into a staging texture for each after the
/// game has drawn and before it draws any layer. The first `layer_capture_count` entries of `layer_captures` are
/// the ones with mod layers: the overlay draws `layers` up to the first layer whose owner is
/// `AmmHostLayer::MANAGER`, copies the back buffer for each, then draws the rest. The manager's layer (the panel
/// and the toasts) is always the last layer, so the copy shows the layers of all mods and not the manager's. With
/// no mod layer in the frame, the layer captures are copied at the same point as the game-only ones. Both sets
/// share the `AMM_HOST_MAX_CAPTURES` staging textures: together they hold at most that many sequences. Each
/// sequence is given once, in one set. When the copy can be read, the overlay asks for a buffer with
/// `amm_host_capture_buffer`, copies the rows into it and calls `amm_host_capture_done`; a capture it cannot copy
/// it reports with `ok` 0. A capture not reported within 30 frames fails.
///
/// The first `stream_count` entries of `streams` are capture stream copies. They are always game-only: the overlay
/// copies the back buffer for each at the same point as the game-only captures, before any layer, into its own
/// `AMM_HOST_MAX_STREAM_FRAMES` stream staging textures, apart from the screenshot ones. When a copy can be read
/// the overlay asks for a buffer with `amm_host_stream_buffer`, copies the rows into it and calls
/// `amm_host_stream_done`; a copy it cannot make it reports with `ok` 0. A copy not reported within 30 frames fails.
/// `release_stream` is true once after the stream ended: the overlay drops its stream staging textures, with any copy
/// in them, before it copies this frame's `streams`.
///
/// `game_picture` is true when a layer of this frame draws the game picture (`GamePicture::HOST_TEXTURE`). The
/// overlay then copies the back buffer into its game picture texture at the same point as the game-only captures,
/// and draws that texture for the commands that name it. Only a mod that holds `Overlay` keeps such a command;
/// it is dropped from every other layer.
///
/// `size_message_width` x `size_message_height`, when not 0 x 0, is a size the overlay posts to the game window at once
/// as `WM_SIZE` (`SIZE_RESTORED`), so the game sets its viewport for that size and calls `ResizeBuffers`: a forced
/// render size, or the window's own size when a forced size ended. Every `ResizeBuffers` of the game chain asks
/// `amm_host_render_size` for the size to use.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmHostFrame {
    pub layers: *const AmmHostLayer,
    pub layer_count: u32,
    pub freed_owners: u32,
    pub cursor_free: bool,
    pub cursor: AmmCursorShape,
    pub hide_vanilla_hud: bool,
    pub capture_count: u32,
    pub captures: [u32; AMM_HOST_MAX_CAPTURES],
    pub layer_capture_count: u32,
    pub layer_captures: [u32; AMM_HOST_MAX_CAPTURES],
    pub game_picture: bool,
    pub stream_count: u32,
    pub streams: [u32; AMM_HOST_MAX_STREAM_FRAMES],
    pub release_stream: bool,
    pub size_message_width: u32,
    pub size_message_height: u32,
}

impl AmmHostFrame {
    /// No layers, the cursor grabbed, the HUD shown, no capture, no stream copy, no game picture and no size message.
    pub fn empty() -> Self {
        Self {
            layers: std::ptr::null(),
            layer_count: 0,
            freed_owners: 0,
            cursor_free: false,
            cursor: AmmCursorShape::Default,
            hide_vanilla_hud: false,
            capture_count: 0,
            captures: [0; AMM_HOST_MAX_CAPTURES],
            layer_capture_count: 0,
            layer_captures: [0; AMM_HOST_MAX_CAPTURES],
            game_picture: false,
            stream_count: 0,
            streams: [0; AMM_HOST_MAX_STREAM_FRAMES],
            release_stream: false,
            size_message_width: 0,
            size_message_height: 0,
        }
    }

    /// This frame with the size message `size`, or none.
    pub fn with_size_message(self, size: Option<RenderSize>) -> Self {
        let size = size.unwrap_or_default();
        Self { size_message_width: size.width, size_message_height: size.height, ..self }
    }

    /// This frame with the two capture sets and the stream copies of `offered` in place of its own.
    pub fn with_captures_of(self, offered: &AmmHostFrame) -> Self {
        Self {
            capture_count: offered.capture_count,
            captures: offered.captures,
            layer_capture_count: offered.layer_capture_count,
            layer_captures: offered.layer_captures,
            stream_count: offered.stream_count,
            streams: offered.streams,
            release_stream: offered.release_stream,
            ..self
        }
    }
}
