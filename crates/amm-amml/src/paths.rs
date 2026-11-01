use std::path::{Path, PathBuf};

use amml_sdk::{Api, SdkError};

use crate::module_path::ModulePath;

/// What the manager host starts with: its config folder, the game folder its `logs/amm` folder goes in, and the
/// version the panel shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagerPaths {
    pub config_dir: PathBuf,
    pub game_dir: PathBuf,
    pub version: String,
}

impl ManagerPaths {
    pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    pub fn new(config_dir: PathBuf, game_dir: PathBuf, version: String) -> Self {
        Self { config_dir, game_dir, version }
    }

    /// The config folder AMML gives `module`, the folder of the game executable, and this build's version.
    pub fn from_api(api: &Api, module: &str) -> Result<Self, SdkError> {
        let config_dir = PathBuf::from(api.mod_dir(module, 1)?);
        let game_dir = ModulePath::of(None)
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .ok_or_else(|| SdkError::Mod("the game folder could not be read".to_string()))?;
        Ok(Self::new(config_dir, game_dir, Self::VERSION.to_string()))
    }
}
