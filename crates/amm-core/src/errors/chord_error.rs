#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordError {
    #[error("not a chord: {0}")]
    Invalid(String),
}
