use std::path::PathBuf;

use crate::services::write::WriteMergeTable;
use crate::structs::settings::HclDocument;
use crate::structs::write::{SaveStatus, WriteRequest};

/// The config writes the manager hands out: the merge table of queued writes, the manager's own config document,
/// and when the writer last refused a write. It shows no toast: each method that can refuse returns false, and the
/// caller shows the save-failed toast. Bounds: the merge table's capacity.
pub struct SaveQueue {
    config_file: Option<(PathBuf, HclDocument)>,
    writes: WriteMergeTable,
    // when the writer last refused a write, for the save status
    refused_ms: Option<u64>,
}

impl SaveQueue {
    /// How long the save status says "Saved" after a write goes out.
    pub const SAVED_SHOW_MS: u64 = 2000;

    pub fn new(config_file: Option<(PathBuf, HclDocument)>, capacity: usize, merge_ms: u64) -> Self {
        Self { config_file, writes: WriteMergeTable::new(capacity, merge_ms), refused_ms: None }
    }

    /// Queues one write. False when the merge table refused it.
    pub fn queue(&mut self, path: PathBuf, text: String, now_ms: u64) -> bool {
        self.writes.queue(path, text, now_ms)
    }

    /// Edits the manager's own config document and queues its write, when the manager has a config file. False
    /// when the merge table refused the write.
    pub fn edit_config(&mut self, edit: impl FnOnce(&HclDocument) -> HclDocument) -> bool {
        let Some((path, document)) = self.config_file.take() else {
            return true;
        };
        let next = edit(&document);
        let text = next.text();
        self.config_file = Some((path.clone(), next));
        self.queue(path, text, 0)
    }

    /// A write the config writer could not take goes back into the merge table, and the save status says "Save
    /// failed" until the next write goes out. False when the merge table refused it again.
    pub fn refused(&mut self, write: WriteRequest, now_ms: u64) -> bool {
        curia::warn!("config write refused by the writer, kept to try again", { path: write.path.display().to_string() });
        let queued = self.queue(write.path, write.text, now_ms);
        self.refused_ms = Some(now_ms);
        queued
    }

    /// A change that could not be written: the save status says "Save failed" until the next write goes out.
    pub fn mark_refused(&mut self, now_ms: u64) {
        self.refused_ms = Some(now_ms);
    }

    /// The save status at `now_ms`, with `drafts_pending` true while an edit waits for its window.
    pub fn status(&self, drafts_pending: bool, now_ms: u64) -> SaveStatus {
        let written = self.writes.last_written_ms();
        if self.refused_ms.is_some_and(|refused| written.is_none_or(|written| refused >= written)) {
            return SaveStatus::Failed;
        }
        if drafts_pending || self.writes.is_pending() {
            return SaveStatus::Saving;
        }
        match written {
            Some(written) if now_ms < written.saturating_add(Self::SAVED_SHOW_MS) => SaveStatus::Saved,
            _ => SaveStatus::Idle,
        }
    }

    /// The merged writes whose window has passed, each taken out of the merge table.
    pub fn take_due(&mut self, now_ms: u64) -> impl Iterator<Item = WriteRequest> + '_ {
        self.writes.take_due(now_ms)
    }
}
