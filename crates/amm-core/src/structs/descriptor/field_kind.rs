use serde::Deserialize;

use crate::structs::descriptor::{ColorEncoding, EnumOption};

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FieldKind {
    Bool,
    Int { min: i64, max: i64, step: i64 },
    Float { min: f64, max: f64, step: f64, unit: Option<String> },
    Enum { options: Vec<EnumOption> },
    Color { alpha: bool, encoding: ColorEncoding },
    Chord,
}
