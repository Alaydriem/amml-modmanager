use crate::structs::icon::{IconEntry, ModIcon};
use crate::structs::registry::ModIndex;

/// One icon per mod slot. Bound: the slot count; at most `slots × IconDecoder::MAX_SIDE² × 4` bytes. A set
/// replaces, a clear or forget frees.
pub struct IconBook {
    entries: Vec<Option<IconEntry>>,
    next_generation: u64,
}

impl IconBook {
    pub fn new(slots: usize) -> Self {
        Self { entries: (0..slots).map(|_| None).collect(), next_generation: 0 }
    }

    /// Replaces the icon of a slot, or frees it for `None`. A slot out of range is ignored.
    pub fn set(&mut self, index: ModIndex, icon: Option<ModIcon>) {
        let Some(entry) = self.entries.get_mut(usize::from(index.0)) else {
            return;
        };
        match icon {
            Some(icon) => {
                self.next_generation += 1;
                *entry = Some(IconEntry { icon, generation: self.next_generation });
            }
            None => *entry = None,
        }
    }

    pub fn get(&self, index: ModIndex) -> Option<&IconEntry> {
        self.entries.get(usize::from(index.0)).and_then(Option::as_ref)
    }

    /// Frees the icon of a slot. A slot out of range is ignored.
    pub fn forget(&mut self, index: ModIndex) {
        if let Some(entry) = self.entries.get_mut(usize::from(index.0)) {
            *entry = None;
        }
    }

    /// The pixel bytes held now.
    pub fn resident_bytes(&self) -> usize {
        self.entries.iter().flatten().map(|entry| entry.icon.bytes()).sum()
    }

    /// The count of slots that hold an icon.
    pub fn len(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
