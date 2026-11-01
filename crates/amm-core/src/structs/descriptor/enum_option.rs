use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct EnumOption {
    pub value: hcl::Value,
    pub label: String,
}
