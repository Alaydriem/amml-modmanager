use amm_core::structs::descriptor::{Capability, Chord, KeyScope};
use amm_core::structs::registry::ModIndex;

use crate::model::RebindTarget;

/// One row of the all-keybinds page: whose bind it is, its label, its chord now, the capability and
/// scope it declares (none for the panel toggle), whether that capability is claimed exclusively,
/// and the mod whose bind shares its chord. The panel toggle's row has an empty name and label: the
/// view gives it the "Mod Manager" and "Open/close panel" messages.
#[derive(Debug, Clone, PartialEq)]
pub struct KeybindRow<'a> {
    pub target: RebindTarget,
    pub mod_name: &'a str,
    pub label: &'a str,
    pub chord: Chord,
    pub capability: Option<Capability>,
    pub scope: KeyScope,
    pub exclusive: bool,
    pub clash_with: Option<ModIndex>,
}
