use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::registry::ModIndex;

use crate::types::{AmmCapability, AmmEvent, AmmEventKind, AmmStatus};

const PHASE_PRESSED: u8 = 0;
const PHASE_RELEASED: u8 = 1;
const PHASE_REPEAT: u8 = 2;
const KEY_DOWN_BIT: u8 = 1;
const KEY_REPEAT_BIT: u8 = 2;

impl AmmEvent {
    fn bare(kind: AmmEventKind) -> Self {
        Self { kind, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 }
    }

    fn with_capability(kind: AmmEventKind, capability: Capability) -> Self {
        Self { capability: AmmCapability::from(capability) as u8, ..Self::bare(kind) }
    }

    fn capability_of(&self) -> Result<Capability, AmmStatus> {
        AmmCapability::try_from(self.capability).map(Capability::from)
    }
}

impl From<ModEvent> for AmmEvent {
    fn from(value: ModEvent) -> Self {
        match value {
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
            ModEvent::Key { vk, down, repeat } => {
                let phase = if down { KEY_DOWN_BIT } else { 0 } | if repeat { KEY_REPEAT_BIT } else { 0 };
                Self { code: vk, phase, ..Self::bare(AmmEventKind::Key) }
            }
            ModEvent::PointerButton { button, down, x, y } => {
                Self { code: u32::from(button), phase: u8::from(down), x, y, ..Self::bare(AmmEventKind::PointerButton) }
            }
            ModEvent::PointerDelta { dx, dy } => Self { x: dx, y: dy, ..Self::bare(AmmEventKind::PointerDelta) },
            ModEvent::Wheel { delta } => Self { x: delta, ..Self::bare(AmmEventKind::Wheel) },
            ModEvent::ScreenshotTaken { sequence, ok } => {
                Self { code: sequence, phase: u8::from(ok), ..Self::bare(AmmEventKind::ScreenshotTaken) }
            }
            ModEvent::ScreenshotWritten { sequence, ok } => {
                Self { code: sequence, phase: u8::from(ok), ..Self::bare(AmmEventKind::ScreenshotWritten) }
            }
            ModEvent::MenuEntryClicked { entry } => {
                Self { code: u32::from(entry), ..Self::bare(AmmEventKind::MenuEntryClicked) }
            }
            ModEvent::LocaleChanged => Self::bare(AmmEventKind::LocaleChanged),
            ModEvent::FakeKeysReleased => Self::bare(AmmEventKind::FakeKeysReleased),
        }
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
            AmmEventKind::Key => ModEvent::Key {
                vk: value.code,
                down: value.phase & KEY_DOWN_BIT != 0,
                repeat: value.phase & KEY_REPEAT_BIT != 0,
            },
            AmmEventKind::PointerButton => ModEvent::PointerButton {
                button: u8::try_from(value.code).map_err(|_| AmmStatus::InvalidArgument)?,
                down: value.phase != 0,
                x: value.x,
                y: value.y,
            },
            AmmEventKind::PointerDelta => ModEvent::PointerDelta { dx: value.x, dy: value.y },
            AmmEventKind::Wheel => ModEvent::Wheel { delta: value.x },
            AmmEventKind::ScreenshotTaken => ModEvent::ScreenshotTaken { sequence: value.code, ok: value.phase != 0 },
            AmmEventKind::ScreenshotWritten => {
                ModEvent::ScreenshotWritten { sequence: value.code, ok: value.phase != 0 }
            }
            AmmEventKind::MenuEntryClicked => {
                ModEvent::MenuEntryClicked { entry: u16::try_from(value.code).map_err(|_| AmmStatus::InvalidArgument)? }
            }
            AmmEventKind::LocaleChanged => ModEvent::LocaleChanged,
            AmmEventKind::FakeKeysReleased => ModEvent::FakeKeysReleased,
        })
    }
}
