use std::path::PathBuf;

use amml_api::Dirs;

use crate::errors::PathsError;

/// What the manager host starts with: its config folder, the data folder its session logs go in (as
/// `data_dir/logs/amm/...`), the game instance folder when the loader knows one, and the version the panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerPaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub game_dir: Option<PathBuf>,
    pub version: String,
}

impl ManagerPaths {
    pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    pub fn new(config_dir: PathBuf, data_dir: PathBuf, game_dir: Option<PathBuf>, version: String) -> Self {
        Self { config_dir, data_dir, game_dir, version }
    }

    /// The folders the loader gives the mod: `config_dir`, `data_dir` and `instance_dir`. A missing config or data
    /// folder is an error; a missing instance folder is not, because the host does not need it.
    pub fn from_dirs(dirs: &Dirs) -> Result<Self, PathsError> {
        let config_dir = dirs.config_dir().ok_or(PathsError::MissingFolder { folder: "config" })?;
        let data_dir = dirs.data_dir().ok_or(PathsError::MissingFolder { folder: "data" })?;
        Ok(Self::new(config_dir, data_dir, dirs.instance_dir(), Self::VERSION.to_string()))
    }
}
