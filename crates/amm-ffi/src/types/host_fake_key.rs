/// One fake key press for the game: key `vk` goes down (`down` 1) or up (`down` 0).
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmHostFakeKey {
    pub vk: u32,
    pub down: u8,
}
