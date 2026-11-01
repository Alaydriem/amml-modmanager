use hcl::Map;
use serde::Deserialize;

use crate::structs::descriptor::raw::{RawClaim, RawKeybind, RawSection};

// The HCL shape of a descriptor: labelled blocks arrive as maps keyed by their label.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RawDescriptor {
    pub namespace: String,
    pub name: String,
    pub display_name: String,
    pub version: String,
    pub author: String,
    pub config_path: String,
    #[serde(default)]
    pub texture_budget_mb: Option<u32>,
    #[serde(default)]
    pub texture_budget_count: Option<u32>,
    #[serde(default)]
    pub claim: Map<String, RawClaim>,
    #[serde(default)]
    pub keybind: Map<String, RawKeybind>,
    #[serde(default)]
    pub input_passthrough: Vec<u32>,
    #[serde(default)]
    pub section: Map<String, RawSection>,
}
