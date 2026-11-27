use amm_abi::types::{AMM_ABI_MINOR, AmmCoreTable, AmmKeybindsTable, AmmLayersTable, AmmLeasesTable, AmmSettingsTable};
use amml_api::{ApiError, Registry};

use super::{
    amm_acquire, amm_bind_down, amm_chord_text, amm_config_document, amm_game_picture, amm_held_by_other, amm_holder,
    amm_locale, amm_panel_open, amm_poll_events, amm_register, amm_register_menu_entry, amm_release, amm_render_size,
    amm_set_callbacks, amm_set_cursor_mode, amm_set_game_viewport, amm_set_icon, amm_set_render_size, amm_set_setting,
    amm_texture_budget, amm_unregister,
};

static CORE: AmmCoreTable = AmmCoreTable {
    size: size_of::<AmmCoreTable>() as u32,
    version: AMM_ABI_MINOR,
    register: Some(amm_register),
    unregister: Some(amm_unregister),
    poll_events: Some(amm_poll_events),
    locale: Some(amm_locale),
    panel_open: Some(amm_panel_open),
    register_menu_entry: Some(amm_register_menu_entry),
    set_icon: Some(amm_set_icon),
};

static SETTINGS: AmmSettingsTable = AmmSettingsTable {
    size: size_of::<AmmSettingsTable>() as u32,
    version: AMM_ABI_MINOR,
    config_document: Some(amm_config_document),
    set_setting: Some(amm_set_setting),
};

static KEYBINDS: AmmKeybindsTable = AmmKeybindsTable {
    size: size_of::<AmmKeybindsTable>() as u32,
    version: AMM_ABI_MINOR,
    chord_text: Some(amm_chord_text),
    bind_down: Some(amm_bind_down),
};

static LAYERS: AmmLayersTable = AmmLayersTable {
    size: size_of::<AmmLayersTable>() as u32,
    version: AMM_ABI_MINOR,
    set_callbacks: Some(amm_set_callbacks),
    texture_budget: Some(amm_texture_budget),
    set_cursor_mode: Some(amm_set_cursor_mode),
    game_picture: Some(amm_game_picture),
    set_game_viewport: Some(amm_set_game_viewport),
    set_render_size: Some(amm_set_render_size),
    render_size: Some(amm_render_size),
};

static LEASES: AmmLeasesTable = AmmLeasesTable {
    size: size_of::<AmmLeasesTable>() as u32,
    version: AMM_ABI_MINOR,
    acquire: Some(amm_acquire),
    release: Some(amm_release),
    holder: Some(amm_holder),
    held_by_other: Some(amm_held_by_other),
};

/// The five registry tables of the manager. They are statics of the manager DLL, which the loader never unmaps, so a
/// consumer can read them for the life of the process; after `HostRuntime::disable_tables` every function refuses.
pub struct ManagerTables;

impl ManagerTables {
    pub const CORE: &'static AmmCoreTable = &CORE;
    pub const SETTINGS: &'static AmmSettingsTable = &SETTINGS;
    pub const KEYBINDS: &'static AmmKeybindsTable = &KEYBINDS;
    pub const LAYERS: &'static AmmLayersTable = &LAYERS;
    pub const LEASES: &'static AmmLeasesTable = &LEASES;

    /// Provides the five tables, core first. Legal only in the mod's `load` or `enable`. The first refusal stops it:
    /// `Refused` when another mod holds a name at this major, as a second copy of the manager would.
    pub fn provide_all(registry: &Registry) -> Result<(), ApiError> {
        registry.provide(Self::CORE)?;
        registry.provide(Self::SETTINGS)?;
        registry.provide(Self::KEYBINDS)?;
        registry.provide(Self::LAYERS)?;
        registry.provide(Self::LEASES)
    }
}
