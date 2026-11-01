use amm_core::structs::registry::ModIndex;

/// What the right pane of the panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelPage {
    Mod(ModIndex),
    Conflicts,
    Keybinds,
    ManagerSettings,
}
