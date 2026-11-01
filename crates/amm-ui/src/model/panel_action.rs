use amm_core::config::{ExclusiveResolution, PairKey, SuspensionResolution};
use amm_core::structs::descriptor::Chord;
use amm_core::structs::menu::MenuEntryId;
use amm_core::structs::registry::ModIndex;

use crate::model::PanelTab;

/// What the operator asked for in the panel. The host applies each action to the manager.
/// `Select(None)` shows the Conflicts page. `Rebind` names the keybind by its position in the
/// descriptor's keybind list; `RebindToggle` is the new chord of the panel toggle.
#[derive(Debug, Clone, PartialEq)]
pub enum PanelAction {
    Open,
    Close,
    SetEnabled(ModIndex, bool),
    Edit {
        mod_index: ModIndex,
        path: String,
        value: hcl::Value,
    },
    Rebind {
        mod_index: ModIndex,
        action: u16,
        chord: Chord,
    },
    RebindToggle(Chord),
    SetExclusive(PairKey, ExclusiveResolution),
    SetSuspension(PairKey, SuspensionResolution),
    Select(Option<ModIndex>),
    SelectTab(PanelTab),
    /// The person clicked menu entry `id` of the mod in `owner` in the corner row.
    MenuEntry {
        owner: ModIndex,
        id: MenuEntryId,
    },
    /// The person picked a language on the Manager settings page: a language code, empty for English.
    SetLanguage(String),
}
