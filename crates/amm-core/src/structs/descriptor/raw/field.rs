use serde::Deserialize;

use crate::structs::descriptor::{ApplyMode, FieldKind, SettingField};

#[derive(Debug, Deserialize)]
pub struct RawField {
    pub label: String,
    #[serde(default)]
    pub help: String,
    pub kind: FieldKind,
    pub default: hcl::Value,
    pub apply: ApplyMode,
}

impl RawField {
    pub fn into_field(self, path: String) -> SettingField {
        SettingField {
            path,
            label: self.label,
            help: self.help,
            kind: self.kind,
            default: self.default,
            apply: self.apply,
        }
    }
}
