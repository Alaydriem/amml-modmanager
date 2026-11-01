use crate::structs::menu::MenuEntryId;
use crate::structs::registry::ModIndex;

/// What one button of the corner row opens: the manager panel, or a mod's own entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CornerTarget {
    Mods,
    Entry { owner: ModIndex, id: MenuEntryId },
}
