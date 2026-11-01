use crate::structs::clash::Clash;
use crate::structs::registry::ModIndex;

/// The clashes of the enabled mods. `conflicts` hold exclusive and key clashes, `interactions` hold
/// suspensions. Both are rebuilt by each analysis and bounded by the mod limit: at most one entry per
/// capability and mod pair, plus a capped number of key clashes.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ClashReport {
    pub conflicts: Vec<Clash>,
    pub interactions: Vec<Clash>,
    // key clashes were left out because the cap was reached
    pub truncated: bool,
}

impl ClashReport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.conflicts.clear();
        self.interactions.clear();
        self.truncated = false;
    }

    /// The unreviewed conflicts that name this mod.
    pub fn badge_count(&self, index: ModIndex) -> usize {
        self.conflicts.iter().filter(|clash| !clash.reviewed && clash.kind.involves(index)).count()
    }

    /// The unreviewed conflicts.
    pub fn conflict_count(&self) -> usize {
        self.conflicts.iter().filter(|clash| !clash.reviewed).count()
    }
}
