use std::ffi::OsString;
use std::fs;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};

use amm_core::errors::StoreError;
use amm_core::structs::settings::HclDocument;

use crate::structs::ReadOutcome;

pub struct ConfigFile;

impl ConfigFile {
    /// Reads and parses a config file. A file that is not valid UTF-8 HCL is copied to `<name>.invalid`, which
    /// replaces an older copy, and reads as `Invalid`. A file that cannot be read, or whose copy fails, reads as
    /// `Unreadable`.
    pub fn read(path: &Path) -> ReadOutcome {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return ReadOutcome::Missing,
            Err(_) => return ReadOutcome::Unreadable,
        };
        let parsed = String::from_utf8(bytes).ok().and_then(|text| HclDocument::new(&text).ok());
        match parsed {
            Some(document) => ReadOutcome::Parsed(document),
            None => match fs::copy(path, Self::sibling(path, ".invalid")) {
                Ok(_) => ReadOutcome::Invalid,
                Err(_) => ReadOutcome::Unreadable,
            },
        }
    }

    /// Writes the text to `<name>.tmp`, flushes it to disk, then renames it over the target.
    pub fn write_atomic(path: &Path, text: &str) -> Result<(), StoreError> {
        if let Some(parent) = path.parent().filter(|parent| !parent.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|_| Self::io(parent))?;
        }
        let tmp = Self::sibling(path, ".tmp");
        let written = fs::File::create(&tmp).and_then(|mut file| {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        });
        if written.is_err() {
            let _ = fs::remove_file(&tmp);
            return Err(Self::io(&tmp));
        }
        fs::rename(&tmp, path).map_err(|_| {
            let _ = fs::remove_file(&tmp);
            Self::io(path)
        })
    }

    pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
        let mut name = path.file_name().map(OsString::from).unwrap_or_default();
        name.push(suffix);
        path.with_file_name(name)
    }

    fn io(path: &Path) -> StoreError {
        StoreError::Io { path: path.to_path_buf() }
    }
}
