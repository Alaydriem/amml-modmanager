use hcl::Value;

use crate::services::settings::{ChordText, ColorCodec};
use crate::structs::descriptor::{ColorEncoding, FieldKind, SettingField, SettingsSection};
use crate::structs::settings::{CheckedConfig, ConfigWarning, HclDocument, ValueFault};

/// Checks config values against a descriptor's settings schema. A number outside its range is clamped; any
/// other bad or missing value takes the field's default.
pub struct SchemaChecker;

impl SchemaChecker {
    pub fn check(sections: &[SettingsSection], doc: HclDocument) -> CheckedConfig {
        let mut document = doc;
        let mut warnings = Vec::new();
        for field in sections.iter().flat_map(|section| &section.fields) {
            let (replacement, warning) = match document.get(&field.path) {
                Some(value) => match Self::resolve(field, &value) {
                    (_, None) => continue,
                    (replacement, Some(warning)) => (replacement, warning),
                },
                None => (field.default.clone(), ConfigWarning::Defaulted { path: field.path.clone() }),
            };
            document = document.with(&field.path, &replacement);
            warnings.push(warning);
        }
        CheckedConfig { document, warnings }
    }

    /// The value when it fits the field as it is, or the warning that says how the check would replace it.
    pub fn check_value(field: &SettingField, v: &Value) -> Result<Value, ConfigWarning> {
        match Self::resolve(field, v) {
            (value, None) => Ok(value),
            (_, Some(warning)) => Err(warning),
        }
    }

    /// The value to keep for the field and, when it is not `v` itself, the warning that says why.
    pub fn resolve(field: &SettingField, v: &Value) -> (Value, Option<ConfigWarning>) {
        let Some(fault) = Self::fault(field, v) else {
            return (v.clone(), None);
        };
        let clamped = match (fault, &field.kind) {
            (ValueFault::OutOfRange, FieldKind::Int { min, max, .. }) => {
                let number = Self::integer(v).map(|number| number.clamp(i128::from(*min), i128::from(*max)));
                number.and_then(|number| i64::try_from(number).ok()).map(Value::from)
            }
            (ValueFault::OutOfRange, FieldKind::Float { min, max, .. }) => {
                v.as_f64().map(|number| Value::from(number.clamp(*min, *max)))
            }
            _ => None,
        };
        match clamped {
            Some(value) => (value, Some(ConfigWarning::Clamped { path: field.path.clone() })),
            None => (field.default.clone(), Some(ConfigWarning::Defaulted { path: field.path.clone() })),
        }
    }

    /// Why a value does not fit the field, or `None` when it does.
    pub fn fault(field: &SettingField, v: &Value) -> Option<ValueFault> {
        let in_range = match &field.kind {
            FieldKind::Bool => return if v.as_bool().is_some() { None } else { Some(ValueFault::WrongType) },
            FieldKind::Int { min, max, .. } => {
                let Some(number) = Self::integer(v) else {
                    return Some(ValueFault::WrongType);
                };
                (i128::from(*min)..=i128::from(*max)).contains(&number)
            }
            FieldKind::Float { min, max, .. } => {
                let Some(number) = v.as_f64() else {
                    return Some(ValueFault::WrongType);
                };
                (*min..=*max).contains(&number)
            }
            FieldKind::Enum { options } => options.iter().any(|option| &option.value == v),
            FieldKind::Color { alpha, encoding } => return Self::color_fault(v, *alpha, *encoding),
            FieldKind::Chord => {
                let is_chord = v.as_str().is_some_and(|text| ChordText::parse(text).is_some());
                return if is_chord { None } else { Some(ValueFault::WrongType) };
            }
        };
        if in_range { None } else { Some(ValueFault::OutOfRange) }
    }

    /// A whole number. A float such as 2.0 is not one.
    pub(crate) fn integer(value: &Value) -> Option<i128> {
        let number = value.as_number()?;
        if number.is_f64() {
            return None;
        }
        number.as_i64().map(i128::from).or_else(|| number.as_u64().map(i128::from))
    }

    fn color_fault(v: &Value, alpha: bool, encoding: ColorEncoding) -> Option<ValueFault> {
        let has_unused_alpha =
            !alpha && encoding == ColorEncoding::HexString && v.as_str().is_some_and(|t| t.len() > 7);
        if ColorCodec::to_rgba(v, encoding).is_some() && !has_unused_alpha {
            return None;
        }
        let has_shape = match encoding {
            ColorEncoding::PackedRgbaU32 => Self::integer(v).is_some(),
            ColorEncoding::FloatArray4 => {
                Self::array4(v).is_some_and(|parts| parts.iter().all(|p| p.as_f64().is_some()))
            }
            ColorEncoding::ByteArray4 => {
                Self::array4(v).is_some_and(|parts| parts.iter().all(|p| Self::integer(p).is_some()))
            }
            ColorEncoding::HexString => false,
        };
        if has_shape { Some(ValueFault::OutOfRange) } else { Some(ValueFault::WrongType) }
    }

    fn array4(value: &Value) -> Option<&Vec<Value>> {
        value.as_array().filter(|parts| parts.len() == 4)
    }
}
