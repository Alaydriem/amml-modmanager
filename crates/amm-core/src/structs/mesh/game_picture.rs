use super::DrawCmd;

/// The live game picture a mod that holds `Overlay` may draw: a copy of the back buffer after the game has drawn
/// and before any layer, `width` x `height` physical pixels. The mod draws it as a user texture with the reserved
/// id `USER_ID` and uploads nothing; the overlay makes the copy only for a frame whose layers draw it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GamePicture {
    pub width: u32,
    pub height: u32,
}

impl GamePicture {
    /// The user texture id reserved for the game picture. A mod's own user textures use the ids below it.
    pub const USER_ID: u32 = u32::MAX;
    /// The texture id the host sees in a draw command of the game picture.
    pub const HOST_TEXTURE: u64 = DrawCmd::USER_TEXTURE_FLAG | Self::USER_ID as u64;

    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}
