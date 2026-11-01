use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;

/// Who holds each capability, with the holder's id, for `holder` calls from any thread. One entry per
/// capability. A new book replaces the old one when a holder changes; a reader keeps the book it loaded alive for
/// as long as it reads it.
pub struct HolderBook {
    holders: [Option<(ModIndex, Box<str>)>; Capability::ALL.len()],
}

impl HolderBook {
    pub fn new(holders: [Option<(ModIndex, Box<str>)>; Capability::ALL.len()]) -> Self {
        Self { holders }
    }

    pub fn empty() -> Self {
        Self::new(Default::default())
    }

    /// The holder of each capability, in `Capability::ALL` order.
    pub fn indices(&self) -> [Option<ModIndex>; Capability::ALL.len()] {
        std::array::from_fn(|slot| self.holders[slot].as_ref().map(|(index, _)| *index))
    }

    /// The id of the mod that holds `cap`, or `None` when no mod holds it.
    pub fn id(&self, cap: Capability) -> Option<&str> {
        self.holders[cap as usize].as_ref().map(|(_, id)| &**id)
    }
}
