/// What the client flags must be after one tick: `hide_hud` while a `Hud` lease with `hide_vanilla_hud` is active,
/// `mouse_free` while the panel is open or a layer is in cursor mode, and `in_play` while the player is in a world
/// with no game screen on top (the game handles the mouse itself in its menus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlagWant {
    pub hide_hud: bool,
    pub mouse_free: bool,
    pub in_play: bool,
}
