use std::path::PathBuf;

use crate::structs::write::WriteRequest;

/// The write-merge state of one config file: the newest text not yet written, when that text was
/// first queued, and when the file was last written. A burst of queued texts gives one write each
/// merge window at most, and the write carries the newest text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingWrite {
    pub path: PathBuf,
    pub text: Option<String>,
    pub queued_ms: u64,
    pub last_write_ms: Option<u64>,
}

impl PendingWrite {
    pub fn new(path: PathBuf) -> Self {
        Self { path, text: None, queued_ms: 0, last_write_ms: None }
    }

    pub fn queue(&mut self, text: String, now_ms: u64) {
        if self.text.is_none() {
            self.queued_ms = now_ms;
        }
        self.text = Some(text);
    }

    /// The write when one is pending and the merge window since the last write has passed.
    pub fn take_due(&mut self, now_ms: u64, merge_ms: u64) -> Option<WriteRequest> {
        self.text.as_ref()?;
        let earliest =
            self.last_write_ms.map_or(self.queued_ms, |last| last.saturating_add(merge_ms).max(self.queued_ms));
        if now_ms < earliest {
            return None;
        }
        self.last_write_ms = Some(now_ms);
        self.text.take().map(|text| WriteRequest { path: self.path.clone(), text })
    }

    pub fn is_pending(&self) -> bool {
        self.text.is_some()
    }
}
