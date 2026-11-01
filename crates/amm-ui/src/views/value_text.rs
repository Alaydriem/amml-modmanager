use amm_core::services::settings::ColorCodec;
use amm_core::structs::descriptor::{FieldKind, SettingField};
use amml_i18n::text::{Args, Text, UserText};
use amml_ui::AmmlUiMessage;
use amml_ui::widget::DragUnit;

use crate::AmmMessage;

/// A settings value as the panel shows it in a sentence such as "Changed in game to 2.50x", and the
/// unit and decimals of a number field. A float shows the decimals its step implies and its unit; an
/// int shows a whole number.
pub struct ValueText;

impl ValueText {
    const MAX_DECIMALS: usize = 6;

    /// "Changed in game to {value}" for the value the game wrote; an on/off value is a word.
    pub fn changed_in_game(field: &SettingField, value: &hcl::Value) -> Text {
        let args = match (&field.kind, value.as_bool()) {
            (FieldKind::Bool, Some(true)) => Args::new().message("value", AmmMessage::ValueOn),
            (FieldKind::Bool, Some(false)) => Args::new().message("value", AmmMessage::ValueOff),
            _ => Args::new().user("value", UserText::from_model(Self::display(field, value))),
        };
        Text::format(AmmlUiMessage::ChangedInGame, args)
    }

    /// A value that is not on/off, as text.
    pub fn display(field: &SettingField, value: &hcl::Value) -> String {
        match &field.kind {
            FieldKind::Float { step, unit, .. } => match value.as_f64() {
                Some(number) => Self::float(number, *step, unit.as_deref()),
                None => value.to_string(),
            },
            FieldKind::Int { .. } => match value.as_i64() {
                Some(number) => number.to_string(),
                None => value.to_string(),
            },
            FieldKind::Enum { options } => options
                .iter()
                .find(|option| option.value == *value)
                .map_or_else(|| value.to_string(), |o| o.label.clone()),
            FieldKind::Color { encoding, .. } => match ColorCodec::to_rgba(value, *encoding) {
                Some([r, g, b, a]) => format!("#{r:02X}{g:02X}{b:02X}{a:02X}"),
                None => value.to_string(),
            },
            FieldKind::Bool | FieldKind::Chord => match value.as_str() {
                Some(text) => text.to_string(),
                None => value.to_string(),
            },
        }
    }

    /// A float with as many decimals as `step` has, then the unit, joined as `unit` says.
    pub fn float(number: f64, step: f64, unit: Option<&str>) -> String {
        let decimals = Self::decimals(step);
        match unit {
            Some(unit) if !Self::joins(unit) => format!("{number:.decimals$} {unit}"),
            Some(unit) => format!("{number:.decimals$}{unit}"),
            None => format!("{number:.decimals$}"),
        }
    }

    /// The unit of a number field: a one-character unit such as `x` or `%` joins the number; a
    /// longer unit such as `blocks/s` follows a gap.
    pub fn unit(unit: Option<&str>) -> DragUnit {
        match unit {
            None => DragUnit::None,
            Some(unit) => {
                let text = Text::user(UserText::from_model(unit.to_owned()));
                if Self::joins(unit) { DragUnit::Joined(text) } else { DragUnit::Text(text) }
            }
        }
    }

    /// The decimals of a step: 0.05 has 2, 0.5 has 1, 1 and 16 have none.
    pub fn decimals(step: f64) -> usize {
        let mut scaled = step.abs();
        for decimals in 0..Self::MAX_DECIMALS {
            if (scaled - scaled.round()).abs() < 1e-9 * scaled.max(1.0) {
                return decimals;
            }
            scaled *= 10.0;
        }
        Self::MAX_DECIMALS
    }

    fn joins(unit: &str) -> bool {
        unit.chars().count() <= 1
    }
}
