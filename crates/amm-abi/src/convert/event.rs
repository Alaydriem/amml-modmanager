use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::registry::ModIndex;

use crate::types::{AmmCapability, AmmEvent, AmmEventKind, AmmStatus};

const PHASE_PRESSED: u8 = 0;
const PHASE_RELEASED: u8 = 1;
const PHASE_REPEAT: u8 = 2;

impl AmmEvent {
    fn bare(kind: AmmEventKind) -> Self {
        Self { kind, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 }
    }

    // A capability with no ABI value (`Input`, which the manager never leases) is written as a byte no reader takes.
    fn with_capability(kind: AmmEventKind, capability: Capability) -> Self {
        let capability = AmmCapability::try_from(capability).map_or(u8::MAX, |capability| capability as u8);
        Self { capability, ..Self::bare(kind) }
    }

    fn capability_of(&self) -> Result<Capability, AmmStatus> {
        AmmCapability::try_from(self.capability).map(Capability::from)
    }
}

// The loader events a client link adds to the stream have no ABI kind: the manager never queues them.
impl TryFrom<ModEvent> for AmmEvent {
    type Error = AmmStatus;

    fn try_from(value: ModEvent) -> Result<Self, Self::Error> {
        Ok(match value {
            ModEvent::Enabled => Self::bare(AmmEventKind::Enabled),
            ModEvent::Disabled => Self::bare(AmmEventKind::Disabled),
            ModEvent::Granted(capability) => Self::with_capability(AmmEventKind::Granted, capability),
            ModEvent::Revoked { capability, by } => {
                Self { other_mod: by.0, ..Self::with_capability(AmmEventKind::Revoked, capability) }
            }
            ModEvent::Suspended { capability, by } => {
                Self { other_mod: by.0, ..Self::with_capability(AmmEventKind::Suspended, capability) }
            }
            ModEvent::Resumed(capability) => Self::with_capability(AmmEventKind::Resumed, capability),
            ModEvent::CapabilityAvailable(capability) => {
                Self::with_capability(AmmEventKind::CapabilityAvailable, capability)
            }
            ModEvent::Keybind { action, phase } => {
                let phase = match phase {
                    KeybindPhase::Pressed => PHASE_PRESSED,
                    KeybindPhase::Released => PHASE_RELEASED,
                    KeybindPhase::Repeat => PHASE_REPEAT,
                };
                Self { code: u32::from(action), phase, ..Self::bare(AmmEventKind::Keybind) }
            }
            ModEvent::SettingsChanged { paths } => Self { code: paths, ..Self::bare(AmmEventKind::SettingsChanged) },
            ModEvent::WorldReloadApply => Self::bare(AmmEventKind::WorldReloadApply),
            ModEvent::MenuEntryClicked { entry } => {
                Self { code: u32::from(entry), ..Self::bare(AmmEventKind::MenuEntryClicked) }
            }
            ModEvent::LocaleChanged => Self::bare(AmmEventKind::LocaleChanged),
            ModEvent::InputLeaseSuspended
            | ModEvent::InputLeaseResumed
            | ModEvent::InputLeaseLost
            | ModEvent::FakeKeysReleased { .. }
            | ModEvent::ScreenshotTaken { .. }
            | ModEvent::ScreenshotWritten { .. }
            | ModEvent::ManagerGone => return Err(AmmStatus::InvalidArgument),
        })
    }
}

impl TryFrom<&AmmEvent> for ModEvent {
    type Error = AmmStatus;

    fn try_from(value: &AmmEvent) -> Result<Self, Self::Error> {
        Ok(match value.kind {
            AmmEventKind::Enabled => ModEvent::Enabled,
            AmmEventKind::Disabled => ModEvent::Disabled,
            AmmEventKind::Granted => ModEvent::Granted(value.capability_of()?),
            AmmEventKind::Revoked => {
                ModEvent::Revoked { capability: value.capability_of()?, by: ModIndex(value.other_mod) }
            }
            AmmEventKind::Suspended => {
                ModEvent::Suspended { capability: value.capability_of()?, by: ModIndex(value.other_mod) }
            }
            AmmEventKind::Resumed => ModEvent::Resumed(value.capability_of()?),
            AmmEventKind::CapabilityAvailable => ModEvent::CapabilityAvailable(value.capability_of()?),
            AmmEventKind::Keybind => {
                let action = u16::try_from(value.code).map_err(|_| AmmStatus::InvalidArgument)?;
                let phase = match value.phase {
                    PHASE_PRESSED => KeybindPhase::Pressed,
                    PHASE_RELEASED => KeybindPhase::Released,
                    PHASE_REPEAT => KeybindPhase::Repeat,
                    _ => return Err(AmmStatus::InvalidArgument),
                };
                ModEvent::Keybind { action, phase }
            }
            AmmEventKind::SettingsChanged => ModEvent::SettingsChanged { paths: value.code },
            AmmEventKind::WorldReloadApply => ModEvent::WorldReloadApply,
            AmmEventKind::MenuEntryClicked => {
                ModEvent::MenuEntryClicked { entry: u16::try_from(value.code).map_err(|_| AmmStatus::InvalidArgument)? }
            }
            AmmEventKind::LocaleChanged => ModEvent::LocaleChanged,
        })
    }
}
