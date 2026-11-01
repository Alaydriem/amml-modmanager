use std::path::PathBuf;

/// One config file to write: the whole text goes to `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WriteRequest {
    pub path: PathBuf,
    pub text: String,
}
