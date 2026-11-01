use crate::structs::settings::{HclDocument, SettingsDraft};

/// The panel's pending edits of every mod, one `SettingsDraft` per mod slot, and when each slot was
/// last edited. A slot is due for saving once `debounce_ms` has passed since its last edit, so a
/// burst of edits gives one save. Bound: one draft and one edit time per mod slot.
pub struct DraftBook {
    drafts: Vec<SettingsDraft>,
    edited_ms: Vec<Option<u64>>,
    debounce_ms: u64,
}

impl DraftBook {
    pub fn new(slots: usize, debounce_ms: u64) -> Self {
        Self {
            drafts: (0..slots).map(|_| SettingsDraft::new(HclDocument::empty())).collect(),
            edited_ms: vec![None; slots],
            debounce_ms,
        }
    }

    /// The drafts, indexed by mod slot. A free slot holds an empty draft.
    pub fn drafts(&self) -> &[SettingsDraft] {
        &self.drafts
    }

    /// Starts the slot over from a saved document, with no pending edit.
    pub fn reset(&mut self, slot: usize, saved: HclDocument) {
        if let Some(draft) = self.drafts.get_mut(slot) {
            *draft = SettingsDraft::new(saved);
            self.edited_ms[slot] = None;
        }
    }

    /// An operator edit, which restarts the slot's save window.
    pub fn edit(&mut self, slot: usize, path: &str, value: hcl::Value, now_ms: u64) {
        if let Some(draft) = self.drafts.get_mut(slot) {
            *draft = draft.edit(path, value);
            self.edited_ms[slot] = Some(now_ms);
        }
    }

    /// A value the mod changed in play. It does not restart the save window.
    pub fn apply_game_change(&mut self, slot: usize, path: &str, value: hcl::Value) {
        if let Some(draft) = self.drafts.get_mut(slot) {
            *draft = draft.apply_game_change(path, value);
        }
    }

    /// True when the slot has an edit whose save window has passed at `now_ms`.
    pub fn is_due(&self, slot: usize, now_ms: u64) -> bool {
        self.edited_ms
            .get(slot)
            .copied()
            .flatten()
            .is_some_and(|edited| now_ms >= edited.saturating_add(self.debounce_ms))
    }

    /// Takes the slot's edits out: the document to save and the edited paths whose value changed.
    /// The slot then waits for its next edit.
    pub fn take(&mut self, slot: usize) -> Option<(HclDocument, Vec<String>)> {
        let draft = self.drafts.get(slot)?;
        self.edited_ms[slot] = None;
        Some(draft.commit())
    }

    /// True while any slot has an edit waiting for its save window.
    pub fn is_pending(&self) -> bool {
        self.edited_ms.iter().any(Option::is_some)
    }

    pub fn len(&self) -> usize {
        self.drafts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.drafts.is_empty()
    }
}
