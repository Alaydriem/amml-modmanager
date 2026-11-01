use serde::Deserialize;

use crate::config::ModKey;
use crate::structs::descriptor::Capability;

/// A capability and two mod ids in sorted order. Read and written as the attribute name
/// `Capability__first__second`, where each mod id is in the form of `ModKey::attribute`. An attribute name holds
/// no `@` or `/`, and a mod id never holds `_`, so the form is unambiguous.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
#[serde(try_from = "String")]
pub struct PairKey {
    capability: Capability,
    first: String,
    second: String,
}

impl PairKey {
    pub fn new(capability: Capability, a: &str, b: &str) -> PairKey {
        let (first, second) = if a <= b { (a, b) } else { (b, a) };
        PairKey { capability, first: first.to_string(), second: second.to_string() }
    }

    /// The HCL attribute name this key is written under.
    pub fn attribute_name(&self) -> String {
        format!(
            "{}{sep}{}{sep}{}",
            self.capability.name(),
            ModKey::attribute(&self.first),
            ModKey::attribute(&self.second),
            sep = Self::ATTRIBUTE_SEPARATOR
        )
    }

    const ATTRIBUTE_SEPARATOR: &'static str = "__";
}

impl TryFrom<String> for PairKey {
    type Error = String;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        let mut parts = text.split(Self::ATTRIBUTE_SEPARATOR);
        match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(name), Some(a), Some(b), None) => {
                let capability = Capability::from_name(name).ok_or_else(|| format!("unknown capability in {text}"))?;
                match (ModKey::id(a), ModKey::id(b)) {
                    (Some(a), Some(b)) => Ok(PairKey::new(capability, &a, &b)),
                    _ => Err(format!("not a pair key: {text}")),
                }
            }
            _ => Err(format!("not a pair key: {text}")),
        }
    }
}
