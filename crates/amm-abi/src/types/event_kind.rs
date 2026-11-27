/// Which event an `AmmEvent` carries. The doc of `AmmEvent` says which fields each kind uses. The values 10 to 15 are
/// unused (game input and screenshot events before 3.0, now loader events), so an old value is never read as another
/// kind.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmEventKind {
    Enabled = 0,
    Disabled = 1,
    Granted = 2,
    Revoked = 3,
    Suspended = 4,
    Resumed = 5,
    CapabilityAvailable = 6,
    Keybind = 7,
    /// Saved settings of the mod changed. `code` is the number of changed paths; the mod reads the whole
    /// document again with `config_document`.
    SettingsChanged = 8,
    WorldReloadApply = 9,
    /// The person clicked the mod's menu entry whose id is in `code`.
    MenuEntryClicked = 16,
    /// The person picked another language. Only a mod that has called `locale` gets it; the mod calls `locale` again.
    LocaleChanged = 17,
}
