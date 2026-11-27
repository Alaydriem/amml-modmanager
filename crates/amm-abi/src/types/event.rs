use super::AmmEventKind;

/// One event from `poll_events`. Fields a kind does not use are zero.
///
/// - `Granted`, `Resumed`, `CapabilityAvailable`: `capability`.
/// - `Revoked`, `Suspended`: `capability` and `other_mod`, the mod that took the lease.
/// - `Keybind`: `code` is the index into the descriptor keybind list, `phase` is 0 pressed, 1 released, 2 repeat.
/// - `SettingsChanged`: `code` is the number of changed paths. The event does not name them: the mod reads the
///   whole document again with `config_document`.
/// - `MenuEntryClicked`: `code` is the id `register_menu_entry` gave the entry.
/// - `LocaleChanged`: no field; the mod reads the code again with `locale`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AmmEvent {
    pub kind: AmmEventKind,
    pub capability: u8,
    pub other_mod: u8,
    pub phase: u8,
    pub code: u32,
    pub x: f32,
    pub y: f32,
}
