use crate::structs::registry::ModIndex;

/// One keybind of one mod. `action` is the position in the descriptor's keybind list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BindRef {
    pub mod_index: ModIndex,
    pub action: u16,
}
