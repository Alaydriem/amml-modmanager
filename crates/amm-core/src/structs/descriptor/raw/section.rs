use hcl::Map;
use serde::Deserialize;

use crate::structs::descriptor::SettingsSection;
use crate::structs::descriptor::raw::RawField;

#[derive(Debug, Deserialize)]
pub struct RawSection {
    pub title: String,
    #[serde(default)]
    pub field: Map<String, RawField>,
}

impl RawSection {
    pub fn into_section(self, id: String) -> SettingsSection {
        SettingsSection {
            id,
            title: self.title,
            fields: self.field.into_iter().map(|(path, field)| field.into_field(path)).collect(),
        }
    }
}
