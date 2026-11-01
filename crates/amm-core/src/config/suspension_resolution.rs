use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum SuspensionResolution {
    Suspend,
    Coexist,
}

impl SuspensionResolution {
    /// The name this resolution is written as, which deserializing reads back.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Suspend => "Suspend",
            Self::Coexist => "Coexist",
        }
    }
}
