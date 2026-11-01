use crate::errors::ManagerError;
use crate::structs::frame::MenuScreen;
use crate::structs::menu::{MenuEntry, MenuEntryId, MenuScreens};
use crate::structs::registry::ModIndex;

/// The menu entries mods placed in the corner row, in registration order. Bounds: `PER_MOD` entries for one mod and
/// `TOTAL` in all; an entry past either is refused with `MenuEntryLimit`.
pub struct MenuEntries {
    entries: Vec<MenuEntry>,
}

impl MenuEntries {
    pub const PER_MOD: usize = 4;
    pub const TOTAL: usize = 8;
    /// The longest label, in characters.
    pub const LABEL_MAX: usize = 24;
    /// The longest mark, in characters.
    pub const MARK_MAX: usize = 3;

    pub fn new() -> Self {
        Self { entries: Vec::with_capacity(Self::TOTAL) }
    }

    /// Adds an entry for `owner`. The label is 1 to `LABEL_MAX` characters, the mark 1 to `MARK_MAX`, and at least
    /// one screen is set, else `MenuEntry`. The id is one more than the highest id `owner` holds now.
    pub fn add(
        &mut self,
        owner: ModIndex,
        label: &str,
        mark: &str,
        screens: MenuScreens,
    ) -> Result<MenuEntryId, ManagerError> {
        let label_len = label.chars().count();
        let mark_len = mark.chars().count();
        if !(1..=Self::LABEL_MAX).contains(&label_len)
            || !(1..=Self::MARK_MAX).contains(&mark_len)
            || screens.is_empty()
        {
            return Err(ManagerError::MenuEntry);
        }
        let owned = self.entries.iter().filter(|entry| entry.owner == owner);
        if owned.clone().count() >= Self::PER_MOD || self.entries.len() >= Self::TOTAL {
            return Err(ManagerError::MenuEntryLimit);
        }
        let id = MenuEntryId(owned.map(|entry| entry.id.0).max().unwrap_or(0) + 1);
        self.entries.push(MenuEntry { owner, id, label: label.to_owned(), mark: mark.to_owned(), screens });
        Ok(id)
    }

    /// Drops every entry of `owner`.
    pub fn forget(&mut self, owner: ModIndex) {
        self.entries.retain(|entry| entry.owner != owner);
    }

    pub fn contains(&self, owner: ModIndex, id: MenuEntryId) -> bool {
        self.entries.iter().any(|entry| entry.owner == owner && entry.id == id)
    }

    /// The entries that show on `screen`, in registration order.
    pub fn for_screen(&self, screen: MenuScreen) -> impl Iterator<Item = &MenuEntry> {
        self.entries.iter().filter(move |entry| entry.screens.contains(screen))
    }
}

impl Default for MenuEntries {
    fn default() -> Self {
        Self::new()
    }
}
