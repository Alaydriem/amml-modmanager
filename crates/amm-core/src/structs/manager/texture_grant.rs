/// The texture budget the manager gives one mod: at most `bytes` bytes of textures in at most `count` textures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureGrant {
    pub bytes: u64,
    pub count: u32,
}
