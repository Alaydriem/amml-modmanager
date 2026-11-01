use crate::structs::descriptor::{ApplyMode, FieldKind};

#[derive(Debug, Clone, PartialEq)]
pub struct SettingField {
    pub path: String,
    pub label: String,
    pub help: String,
    pub kind: FieldKind,
    pub default: hcl::Value,
    pub apply: ApplyMode,
}
