use std::path::PathBuf;

/// Why this copy of the manager stays off at load.
#[derive(Debug, thiserror::Error)]
pub enum DupError {
    #[error("another manager (amml_modmanager.dll or alaydriem-mod-manager.dll) is loaded from {}; this copy stays off", path.display())]
    OtherCopy { path: PathBuf },
    #[error("this copy is not loaded as amml_modmanager.dll, so mods cannot find it; it stays off")]
    NotFound,
    #[error("the module of this copy could not be found; it stays off")]
    OwnModule,
}
