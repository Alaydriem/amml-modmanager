/// Which event an `AmmEvent` carries. The doc of `AmmEvent` says which fields each kind uses.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmEventKind {
    Enabled = 0,
    Disabled,
    Granted,
    Revoked,
    Suspended,
    Resumed,
    CapabilityAvailable,
    Keybind,
    /// Saved settings of the mod changed. `code` is the number of changed paths; the mod reads the whole
    /// document again with `config_document`.
    SettingsChanged,
    WorldReloadApply,
    Key,
    PointerButton,
    PointerDelta,
    Wheel,
    /// The pixels of capture `code` (its sequence) were copied (`phase` 1) or the copy failed or was
    /// cancelled (`phase` 0).
    ScreenshotTaken,
    /// The file of capture `code` (its sequence) is in place (`phase` 1) or the encode or write failed (`phase` 0).
    ScreenshotWritten,
    /// Since 1.4. The person clicked the mod's menu entry whose id is in `code`.
    MenuEntryClicked,
    /// Since 1.5. The person picked another language. Only a mod that has called `locale` gets it; the mod calls
    /// `locale` again.
    LocaleChanged,
    /// Since 2.1. The manager let go of the fake keys the mod held with `send_key`, or (since 2.2) the mouse buttons
    /// it held with `send_pointer`, because the panel opened, the game window lost focus, the world was left or the
    /// mod was disabled. The mod drops its own record of them. No field.
    FakeKeysReleased,
}
