use crate::structs::settings::{ConfigWarning, HclDocument};

/// A config document after the schema check. `warnings` holds at most one entry per schema field, so its bound
/// is the descriptor's field limit.
#[derive(Debug, Clone)]
pub struct CheckedConfig {
    pub document: HclDocument,
    pub warnings: Vec<ConfigWarning>,
}

impl CheckedConfig {
    /// True when the check changed the document, so the file must be written back.
    pub fn changed(&self) -> bool {
        !self.warnings.is_empty()
    }
}
