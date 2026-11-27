use amm_abi::types::AmmCursorShape;
use amm_core::structs::mesh::GamePicture;
use amm_core::structs::render::RenderSize;

use super::AmmHostLayer;

/// The texture id of a draw command that shows the game picture (`GamePicture::HOST_TEXTURE`). The overlay draws its
/// own copy of the back buffer for it; it is never in the texture registry.
pub const AMM_HOST_GAME_PICTURE_TEXTURE: u64 = 0x8000_0000_FFFF_FFFF;

const _: () = assert!(AMM_HOST_GAME_PICTURE_TEXTURE == GamePicture::HOST_TEXTURE);

/// What the overlay draws for one Present: `layer_count` layers at `layers`, bottom to top, valid until the next
/// `amm_host_frame`. `freed_owners` has bit N set when mod slot N left since the last frame, so the overlay drops
/// every texture of that owner. `cursor_free` releases the mouse grab and shows `cursor`; `hide_vanilla_hud`
/// hides the game HUD.
///
/// `game_picture` is true when a layer of this frame draws the game picture (`GamePicture::HOST_TEXTURE`). The
/// overlay then copies the back buffer into its game picture texture after the game has drawn and before any layer,
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
    pub game_picture: bool,
    pub size_message_width: u32,
    pub size_message_height: u32,
}

impl AmmHostFrame {
    /// No layers, the cursor grabbed, the HUD shown, no game picture and no size message.
    pub fn empty() -> Self {
        Self {
            layers: std::ptr::null(),
            layer_count: 0,
            freed_owners: 0,
            cursor_free: false,
            cursor: AmmCursorShape::Default,
            hide_vanilla_hud: false,
            game_picture: false,
            size_message_width: 0,
            size_message_height: 0,
        }
    }

    /// This frame with the size message `size`, or none.
    pub fn with_size_message(self, size: Option<RenderSize>) -> Self {
        let size = size.unwrap_or_default();
        Self { size_message_width: size.width, size_message_height: size.height, ..self }
    }
}
