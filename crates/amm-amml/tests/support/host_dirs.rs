use std::fs;
use std::path::PathBuf;

use amml_modmanager::ManagerPaths;
use tempfile::TempDir;

/// Temporary config and game folders for a manager host, and a config folder for each test mod.
pub struct HostDirs {
    dir: TempDir,
}

impl HostDirs {
    pub fn new() -> Self {
        let dirs = Self { dir: tempfile::tempdir().unwrap() };
        fs::create_dir_all(dirs.paths().config_dir).unwrap();
        fs::create_dir_all(dirs.paths().game_dir).unwrap();
        dirs
    }

    pub fn paths(&self) -> ManagerPaths {
        ManagerPaths::new(
            self.dir.path().join("manager").join("config"),
            self.dir.path().join("game"),
            "0.1.0-test".to_string(),
        )
    }

    /// The config folder of the test mod `id`, created.
    pub fn mod_config(&self, id: &str) -> PathBuf {
        let dir = self.dir.path().join("mods").join(id).join("config");
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
