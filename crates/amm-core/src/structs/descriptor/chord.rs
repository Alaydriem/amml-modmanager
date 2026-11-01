use serde::{Deserialize, Serialize};

use crate::errors::ChordError;
use crate::services::settings::ChordText;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct Chord {
    pub key: u32,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    // F3 held as a modifier; the OS does not report it, so the input router sets it
    pub f3: bool,
}

impl Chord {
    pub const F3: u32 = 0x72;

    /// This chord with F3 held or not. F3 is never its own modifier.
    pub fn with_f3(self, held: bool) -> Self {
        Self { f3: held && self.key != Self::F3, ..self }
    }
}

impl TryFrom<String> for Chord {
    type Error = ChordError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        ChordText::parse(&text).ok_or(ChordError::Invalid(text))
    }
}

impl From<Chord> for String {
    fn from(chord: Chord) -> Self {
        ChordText::format(chord)
    }
}
