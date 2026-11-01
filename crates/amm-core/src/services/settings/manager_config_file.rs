use crate::config::{ExclusiveResolution, ManagerConfig, ModKey, PairKey, SuspensionResolution};
use crate::services::settings::ChordText;
use crate::structs::descriptor::Chord;
use crate::structs::settings::HclDocument;

/// Reads the manager's `config.hcl` and edits it, so the reader and the writer agree on one layout. Edits go
/// through `HclDocument`, which keeps comments, order and every attribute the manager does not write. The manager
/// writes `toggle_chord`, `language`, `enabled.<mod key>`, where `<mod key>` is `ModKey::attribute`,, `resolutions.exclusive.<pair>` and
/// `resolutions.suspension.<pair>`, where `<pair>` is `PairKey::attribute_name`.
pub struct ManagerConfigFile;

impl ManagerConfigFile {
    const TOGGLE_CHORD: &'static str = "toggle_chord";
    const LANGUAGE: &'static str = "language";
    const ENABLED: &'static str = "enabled";
    const EXCLUSIVE: &'static str = "resolutions.exclusive";
    const SUSPENSION: &'static str = "resolutions.suspension";

    /// The config and the document it was read from, with each budget clamped to its bound (one warning per
    /// clamped field). An error names why the text is not a manager config.
    pub fn read(text: &str) -> Result<(ManagerConfig, HclDocument), String> {
        let document = HclDocument::new(text).map_err(|_| "manager config is not valid HCL".to_string())?;
        let config: ManagerConfig = hcl::from_str(text).map_err(|error| error.to_string())?;
        let (budgets, clamped) = config.budgets.clamped();
        for field in clamped {
            curia::warn!("manager budget out of bounds, clamped", { field: field });
        }
        Ok((ManagerConfig { budgets, ..config }, document))
    }

    pub fn with_toggle_chord(document: &HclDocument, chord: Chord) -> HclDocument {
        document.with(Self::TOGGLE_CHORD, &hcl::Value::String(ChordText::format(chord)))
    }

    pub fn with_language(document: &HclDocument, code: &str) -> HclDocument {
        document.with(Self::LANGUAGE, &hcl::Value::String(code.to_owned()))
    }

    pub fn with_enabled(document: &HclDocument, id: &str, enabled: bool) -> HclDocument {
        document.with(&format!("{}.{}", Self::ENABLED, ModKey::attribute(id)), &hcl::Value::Bool(enabled))
    }

    pub fn with_exclusive(document: &HclDocument, key: &PairKey, resolution: &ExclusiveResolution) -> HclDocument {
        let path = format!("{}.{}", Self::EXCLUSIVE, key.attribute_name());
        document.with(&path, &hcl::Value::String(resolution.text()))
    }

    pub fn with_suspension(document: &HclDocument, key: &PairKey, resolution: SuspensionResolution) -> HclDocument {
        let path = format!("{}.{}", Self::SUSPENSION, key.attribute_name());
        document.with(&path, &hcl::Value::String(resolution.name().to_string()))
    }
}
