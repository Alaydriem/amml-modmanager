use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer};

use crate::config::{Budgets, ModKey, ResolutionBook};
use crate::services::settings::ChordText;
use crate::structs::descriptor::Chord;

/// The manager's own `config.hcl`. `menu_button` shows the Mods button while the game shows its cursor.
/// `language` is the language code for every mod: empty for English, `en_XA` for the pseudo-locale, or a locale
/// such as `ru`. An unknown code is kept as it is; each mod shows English for it. A `toggle_chord` that does not
/// parse gives the default chord and a warning, and the rest of the file is still read.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(default)]
pub struct ManagerConfig {
    #[serde(deserialize_with = "ManagerConfig::read_toggle_chord")]
    pub toggle_chord: Chord,
    #[serde(deserialize_with = "ManagerConfig::read_by_mod")]
    pub enabled: BTreeMap<String, bool>,
    pub resolutions: ResolutionBook,
    pub budgets: Budgets,
    pub telemetry: bool,
    pub telemetry_interval_secs: u32,
    pub log_level: String,
    #[serde(deserialize_with = "ManagerConfig::read_by_mod")]
    pub mod_log_levels: BTreeMap<String, String>,
    pub log_sessions_kept: usize,
    pub menu_button: bool,
    pub language: String,
}

impl ManagerConfig {
    const DEFAULT_TOGGLE_KEY: u32 = 0x79;
    const DEFAULT_INTERVAL_SECS: u32 = 10;
    const DEFAULT_LOG_LEVEL: &str = "Info";
    const DEFAULT_SESSIONS_KEPT: usize = 20;
    const LEVELS: [curia::Level; 5] =
        [curia::Level::Error, curia::Level::Warn, curia::Level::Info, curia::Level::Debug, curia::Level::Trace];

    fn read_toggle_chord<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Chord, D::Error> {
        let text = String::deserialize(deserializer)?;
        Ok(ChordText::parse(&text).unwrap_or_else(|| {
            curia::warn!("manager toggle chord not read, default used", { toggle_chord: text });
            Self::default().toggle_chord
        }))
    }

    // The file keys a mod by `ModKey::attribute`; the map is keyed by the id. A key that is not in that form stays as
    // it is, and matches no id.
    fn read_by_mod<'de, D: Deserializer<'de>, V: Deserialize<'de>>(
        deserializer: D,
    ) -> Result<BTreeMap<String, V>, D::Error> {
        let stored = BTreeMap::<String, V>::deserialize(deserializer)?;
        Ok(stored.into_iter().map(|(key, value)| (ModKey::id(&key).unwrap_or(key), value)).collect())
    }

    /// A mod with no stored choice is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.get(id).copied().unwrap_or(true)
    }

    /// The level for a mod: its own entry, else the global level. An unknown name gives `Info`.
    pub fn log_level_for(&self, id: &str) -> curia::Level {
        let name = self.mod_log_levels.get(id).unwrap_or(&self.log_level);
        Self::LEVELS.into_iter().find(|level| level.as_str().eq_ignore_ascii_case(name)).unwrap_or(curia::Level::Info)
    }
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            toggle_chord: Chord { key: Self::DEFAULT_TOGGLE_KEY, ctrl: true, shift: true, alt: false, f3: false },
            enabled: BTreeMap::new(),
            resolutions: ResolutionBook::new(),
            budgets: Budgets::default(),
            telemetry: false,
            telemetry_interval_secs: Self::DEFAULT_INTERVAL_SECS,
            log_level: Self::DEFAULT_LOG_LEVEL.to_string(),
            mod_log_levels: BTreeMap::new(),
            log_sessions_kept: Self::DEFAULT_SESSIONS_KEPT,
            menu_button: true,
            language: String::new(),
        }
    }
}
