use crate::structs::descriptor::{Capability, Chord, KeyScope};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeybindDecl {
    pub action: String,
    pub label: String,
    pub default_chord: Chord,
    pub capability: Capability,
    pub scope: KeyScope,
    pub config_path: String,
}
