use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use amm_core::errors::StoreError;

use crate::logging::SessionStamp;

/// Deletes the oldest sessions of one mod folder, with the archives each session rotated out. Only `<stamp>.log` and
/// Curia archive names `<stamp>_<archive stamp>[-<n>].log` count; every other file is left alone.
pub struct LogRetention;

impl LogRetention {
    /// Keeps the newest `keep` sessions and deletes the files of the rest. Returns how many files were deleted.
    pub fn prune(dir: &Path, keep: usize) -> Result<usize, StoreError> {
        Self::run(dir, keep, None)
    }

    /// As `prune`, and never deletes a file of the `current` session, archives included.
    pub fn prune_keeping(dir: &Path, keep: usize, current: &SessionStamp) -> Result<usize, StoreError> {
        Self::run(dir, keep, Some(current))
    }

    fn run(dir: &Path, keep: usize, current: Option<&SessionStamp>) -> Result<usize, StoreError> {
        let io = || StoreError::Io { path: dir.to_path_buf() };
        let entries = match fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(0),
            Err(_) => return Err(io()),
        };
        let mut sessions: Vec<(SessionStamp, std::path::PathBuf)> = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| io())?;
            let name = entry.file_name();
            if let Some(stamp) = name.to_str().and_then(SessionStamp::from_log_file_name) {
                sessions.push((stamp, entry.path()));
            }
        }
        let mut stamps: Vec<&SessionStamp> = sessions.iter().map(|(stamp, _)| stamp).collect();
        stamps.sort_by(|a, b| b.cmp(a));
        stamps.dedup();
        let doomed: Vec<SessionStamp> = stamps.into_iter().skip(keep).cloned().collect();
        let mut removed = 0;
        for (stamp, path) in sessions {
            if current == Some(&stamp) || !doomed.contains(&stamp) {
                continue;
            }
            fs::remove_file(&path).map_err(|_| StoreError::Io { path })?;
            removed += 1;
        }
        Ok(removed)
    }
}
