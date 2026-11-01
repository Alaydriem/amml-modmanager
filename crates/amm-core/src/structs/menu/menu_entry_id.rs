/// The id of one menu entry, given by the manager at registration. Ids count from 1 within each mod.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MenuEntryId(pub u16);
