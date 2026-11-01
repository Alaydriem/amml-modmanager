use crate::structs::registry::ModMask;
use crate::structs::snapshot::LayerEntry;

/// What the render thread reads for a frame. `layers` is bottom to top: Hud layers in registration
/// order, then Overlay layers of mods without an `Input` lease in registration order, then Overlay
/// layers of `Input` holders with the most recent acquire on top. It holds at most one Hud and one
/// Overlay entry per mod slot, `MAX_LAYERS`. `menu_button` is true while the manager's menu button
/// shows as soon as the game shows its own cursor: the config allows it and the panel is closed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderSnapshot {
    pub layers: Vec<LayerEntry>,
    pub cursor_free: bool,
    pub hide_vanilla_hud: bool,
    pub blocks_player_turn: bool,
    pub panel_open: bool,
    pub menu_button: bool,
}

impl RenderSnapshot {
    pub const MAX_LAYERS: usize = 2 * ModMask::CAPACITY;

    pub fn new() -> Self {
        Self::default()
    }
}
