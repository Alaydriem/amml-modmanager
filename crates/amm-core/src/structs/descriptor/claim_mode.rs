use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum ClaimMode {
    Exclusive,
    Shared,
}
