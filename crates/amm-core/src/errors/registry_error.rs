#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RegistryError {
    #[error("too many mods")]
    TooManyMods,
    #[error("mod id already registered: {0}")]
    DuplicateId(String),
}
