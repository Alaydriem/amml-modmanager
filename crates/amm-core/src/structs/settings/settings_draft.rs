use std::collections::BTreeMap;

use crate::structs::settings::HclDocument;

/// The panel's unsaved edits of one mod's settings, beside the saved document they started from.
///
/// `edited` holds each edited path, with the value a game change wrote to that path while the edit was pending.
/// It is bounded by `MAX_EDITED`, the per-mod settings field limit; an edit of a new path past the bound is
/// dropped. `revert` and a fresh draft after a save clear it.
#[derive(Debug, Clone)]
pub struct SettingsDraft {
    saved: HclDocument,
    draft: HclDocument,
    edited: BTreeMap<String, Option<hcl::Value>>,
}

impl SettingsDraft {
    pub const MAX_EDITED: usize = 256;

    pub fn new(saved: HclDocument) -> SettingsDraft {
        Self { draft: saved.clone(), saved, edited: BTreeMap::new() }
    }

    pub fn edit(&self, path: &str, v: hcl::Value) -> SettingsDraft {
        if !self.edited.contains_key(path) && self.edited.len() >= Self::MAX_EDITED {
            curia::warn!("settings draft is full, edit dropped", { path: path });
            return self.clone();
        }
        let mut edited = self.edited.clone();
        edited.entry(path.to_string()).or_insert(None);
        Self { saved: self.saved.clone(), draft: self.draft.with(path, &v), edited }
    }

    /// A value the mod changed in play. The saved document always takes it. The draft takes it too unless the
    /// operator has an edit of the same path pending; then the draft keeps the edit and the value becomes a note.
    pub fn apply_game_change(&self, path: &str, v: hcl::Value) -> SettingsDraft {
        let saved = self.saved.with(path, &v);
        if self.edited.contains_key(path) {
            let mut edited = self.edited.clone();
            edited.insert(path.to_string(), Some(v));
            return Self { saved, draft: self.draft.clone(), edited };
        }
        Self { saved, draft: self.draft.with(path, &v), edited: self.edited.clone() }
    }

    pub fn game_change_note(&self, path: &str) -> Option<&hcl::Value> {
        self.edited.get(path)?.as_ref()
    }

    /// The draft value at `path`, with the operator's unsaved edit if there is one.
    pub fn value(&self, path: &str) -> Option<hcl::Value> {
        self.draft.get(path)
    }

    /// True when the operator edited `path` and the draft value differs from the saved one.
    pub fn is_edited(&self, path: &str) -> bool {
        self.edited.contains_key(path) && self.is_changed(path)
    }

    pub fn is_dirty(&self) -> bool {
        self.edited.keys().any(|path| self.is_changed(path))
    }

    /// The document to save and the edited paths whose value differs from the saved one.
    pub fn commit(&self) -> (HclDocument, Vec<String>) {
        let paths = self.edited.keys().filter(|path| self.is_changed(path)).cloned().collect();
        (self.draft.clone(), paths)
    }

    pub fn revert(&self) -> SettingsDraft {
        Self::new(self.saved.clone())
    }

    fn is_changed(&self, path: &str) -> bool {
        self.draft.get(path) != self.saved.get(path)
    }
}
