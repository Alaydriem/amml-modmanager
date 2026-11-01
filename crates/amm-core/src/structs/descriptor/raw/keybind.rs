use serde::Deserialize;

use crate::structs::descriptor::{Capability, Chord, KeyScope, KeybindDecl};

#[derive(Debug, Deserialize)]
pub struct RawKeybind {
    pub label: String,
    #[serde(rename = "default")]
    pub default_chord: Chord,
    pub capability: Capability,
    pub scope: KeyScope,
    pub config_path: String,
}

impl RawKeybind {
    pub fn into_decl(self, action: String) -> KeybindDecl {
        KeybindDecl {
            action,
            label: self.label,
            default_chord: self.default_chord,
            capability: self.capability,
            scope: self.scope,
            config_path: self.config_path,
        }
    }
}
