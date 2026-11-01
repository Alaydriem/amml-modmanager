/// One call the fake table's `set_setting` received: the setting path and the HCL expression text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingCall {
    pub path: String,
    pub expression: String,
}
