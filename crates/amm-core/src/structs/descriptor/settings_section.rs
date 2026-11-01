use crate::structs::descriptor::SettingField;

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsSection {
    pub id: String,
    pub title: String,
    pub fields: Vec<SettingField>,
}
