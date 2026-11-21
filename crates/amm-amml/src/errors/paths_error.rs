/// Why the manager folders could not be read from the loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PathsError {
    /// The loader gave no such folder for the mod.
    #[error("the loader gives the manager no {folder} folder")]
    MissingFolder { folder: &'static str },
}
