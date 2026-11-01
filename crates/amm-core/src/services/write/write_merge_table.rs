use std::path::{Path, PathBuf};

use crate::structs::write::{PendingWrite, WriteRequest};

/// The write-merge state of every config file the manager writes. A burst of queued texts for one
/// file gives one write each merge window at most, and that write carries the newest text.
///
/// Bound: `capacity` entries. A pending entry stays until it drains; when the table is full, a new
/// path evicts the idle entry with the oldest write. A table full of pending entries refuses the new
/// write and logs it.
pub struct WriteMergeTable {
    writes: Vec<PendingWrite>,
    capacity: usize,
    merge_ms: u64,
}

impl WriteMergeTable {
    pub fn new(capacity: usize, merge_ms: u64) -> Self {
        Self { writes: Vec::with_capacity(capacity), capacity, merge_ms }
    }

    /// Queues the newest text of a file. Returns false when the table is full of pending writes and
    /// the text was refused.
    pub fn queue(&mut self, path: PathBuf, text: String, now_ms: u64) -> bool {
        match self.entry(&path) {
            Some(entry) => {
                self.writes[entry].queue(text, now_ms);
                true
            }
            None => {
                curia::warn!("config write table is full of pending writes, write refused", {
                    path: path.display().to_string(),
                    bound: self.capacity,
                });
                false
            }
        }
    }

    /// When the newest write went out, or `None` before the first.
    pub fn last_written_ms(&self) -> Option<u64> {
        self.writes.iter().filter_map(|entry| entry.last_write_ms).max()
    }

    /// The merged writes whose window has passed. Each yielded write is taken out of the table.
    pub fn take_due(&mut self, now_ms: u64) -> impl Iterator<Item = WriteRequest> + '_ {
        let merge_ms = self.merge_ms;
        self.writes.iter_mut().filter_map(move |entry| entry.take_due(now_ms, merge_ms))
    }

    /// True while any file has a queued text not yet handed out.
    pub fn is_pending(&self) -> bool {
        self.writes.iter().any(PendingWrite::is_pending)
    }

    pub fn merge_ms(&self) -> u64 {
        self.merge_ms
    }

    // The table position of the entry for `path`. A new path takes a free place, or the place of the
    // idle entry written longest ago. `None` when every entry is pending.
    fn entry(&mut self, path: &Path) -> Option<usize> {
        if let Some(entry) = self.writes.iter().position(|entry| entry.path == path) {
            return Some(entry);
        }
        if self.writes.len() >= self.capacity {
            let idle = (0..self.writes.len())
                .filter(|i| !self.writes[*i].is_pending())
                .min_by_key(|i| self.writes[*i].last_write_ms.unwrap_or(0))?;
            self.writes.swap_remove(idle);
        }
        self.writes.push(PendingWrite::new(path.to_path_buf()));
        Some(self.writes.len() - 1)
    }
}
