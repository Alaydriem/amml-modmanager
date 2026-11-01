use crate::structs::menu::{MenuEntryId, MenuScreens};
use crate::structs::registry::ModIndex;

/// A button a mod placed in the corner row, left of the Mods button: a short mark in a tile and a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuEntry {
    pub owner: ModIndex,
    pub id: MenuEntryId,
    pub label: String,
    pub mark: String,
    pub screens: MenuScreens,
}
