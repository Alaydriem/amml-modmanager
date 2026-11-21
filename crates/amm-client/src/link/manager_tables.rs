use amm_abi::types::{AmmCoreTable, AmmKeybindsTable, AmmLayersTable, AmmLeasesTable, AmmSettingsTable};
use amml_api::{Provided, Registry};

/// The five tables of one connect to the manager, from the registry. The manager keeps them as statics of its DLL,
/// which the loader never unmaps, so they stay readable; after the manager's `ProviderGone` the link stops calling
/// them.
#[derive(Debug, Clone, Copy)]
pub struct ManagerTables {
    pub core: &'static AmmCoreTable,
    pub settings: &'static AmmSettingsTable,
    pub keybinds: &'static AmmKeybindsTable,
    pub layers: &'static AmmLayersTable,
    pub leases: &'static AmmLeasesTable,
}

impl ManagerTables {
    /// The registry names of the five tables, core first.
    pub const NAMES: [&'static str; 5] = [
        AmmCoreTable::NAME,
        AmmSettingsTable::NAME,
        AmmKeybindsTable::NAME,
        AmmLayersTable::NAME,
        AmmLeasesTable::NAME,
    ];

    /// The five tables at this client's major and minor, or the name of the first one the registry does not give.
    pub fn require(registry: &Registry) -> Result<ManagerTables, &'static str> {
        Ok(Self {
            core: registry.require::<AmmCoreTable>().ok_or(AmmCoreTable::NAME)?,
            settings: registry.require::<AmmSettingsTable>().ok_or(AmmSettingsTable::NAME)?,
            keybinds: registry.require::<AmmKeybindsTable>().ok_or(AmmKeybindsTable::NAME)?,
            layers: registry.require::<AmmLayersTable>().ok_or(AmmLayersTable::NAME)?,
            leases: registry.require::<AmmLeasesTable>().ok_or(AmmLeasesTable::NAME)?,
        })
    }

    /// Whether `name` is one of the five table names.
    pub fn is_manager_table(name: &str) -> bool {
        Self::NAMES.contains(&name)
    }
}
