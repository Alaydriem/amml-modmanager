use amm_core::structs::registry::ModIndex;

/// The keybind a rebind waits for: a mod's keybind by its position in the descriptor's keybind
/// list, or the manager's own panel toggle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RebindTarget {
    Mod { mod_index: ModIndex, action: u16 },
    ManagerToggle,
}
