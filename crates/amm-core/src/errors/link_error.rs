use amml_api::ApiError;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("the mod manager is not loaded")]
    NoManager,
    #[error("the mod manager speaks ABI major {found}")]
    AbiMismatch { found: u32 },
    #[error("the mod manager returned status {0}")]
    Status(i32),
    #[error("a JSON document is not valid: {reason}")]
    Json { reason: String },
    #[error("the icon file cannot be used: {reason}")]
    IconFile { reason: String },
    #[error("the link has not connected to the mod manager yet")]
    NotConnected,
    #[error("the mod manager is gone")]
    Gone,
    #[error("the loader refused the call: {0}")]
    Api(ApiError),
    #[error("the mod's uses block has no row for {0}")]
    MissingUse(String),
    #[error("the descriptor names mod {descriptor}, but the loader runs it as {module}")]
    IdMismatch { descriptor: String, module: String },
}
