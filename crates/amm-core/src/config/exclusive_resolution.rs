use serde::Deserialize;

/// How an exclusive clash resolves. Written as `DenySecond` or `Prefer:<mod id>`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub enum ExclusiveResolution {
    DenySecond,
    Prefer(String),
}

impl ExclusiveResolution {
    const DENY_SECOND: &str = "DenySecond";
    const PREFER_PREFIX: &str = "Prefer:";

    /// The text this resolution is written as, which `try_from` reads back.
    pub fn text(&self) -> String {
        match self {
            Self::DenySecond => Self::DENY_SECOND.to_string(),
            Self::Prefer(id) => format!("{}{id}", Self::PREFER_PREFIX),
        }
    }
}

impl TryFrom<String> for ExclusiveResolution {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        if text == Self::DENY_SECOND {
            return Ok(Self::DenySecond);
        }
        match text.strip_prefix(Self::PREFER_PREFIX) {
            Some(id) if !id.is_empty() => Ok(Self::Prefer(id.to_string())),
            _ => Err(format!("unknown exclusive resolution {text}")),
        }
    }
}
