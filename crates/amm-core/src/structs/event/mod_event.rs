use amml_api::ShotId;

use crate::structs::descriptor::Capability;
use crate::structs::event::{KeybindPhase, ReleaseReason};
use crate::structs::registry::ModIndex;

/// One event for one mod. Plain data, so a queue of them is a flat array. The manager queues the variants up to
/// `LocaleChanged`; the variants after it are the loader events a client link passes on in the same stream, and the
/// manager never queues them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ModEvent {
    Enabled,
    Disabled,
    Granted(Capability),
    Revoked {
        capability: Capability,
        by: ModIndex,
    },
    Suspended {
        capability: Capability,
        by: ModIndex,
    },
    Resumed(Capability),
    CapabilityAvailable(Capability),
    Keybind {
        action: u16,
        phase: KeybindPhase,
    },
    SettingsChanged {
        paths: u32,
    },
    WorldReloadApply,
    /// The person clicked the mod's menu entry `entry` in the corner row.
    MenuEntryClicked {
        entry: u16,
    },
    /// The person picked another language in the manager. Only a mod that has read the language code gets it;
    /// the mod reads the code again.
    LocaleChanged,
    /// The link's input lease was suspended by a lease that preempts or outranks it (for example the open panel).
    InputLeaseSuspended,
    /// The link's suspended input lease is active again.
    InputLeaseResumed,
    /// The link's input lease is gone; `acquire(Capability::Input)` takes a new one.
    InputLeaseLost,
    /// The loader let go of the keys, buttons and movement the link held for the mod. The mod drops its own record
    /// of them.
    FakeKeysReleased {
        reason: ReleaseReason,
    },
    /// The image of screenshot `id` was read back, or the copy failed (`ok` false).
    ScreenshotTaken {
        id: ShotId,
        ok: bool,
    },
    /// The file of screenshot `id` is in place, or the write failed (`ok` false).
    ScreenshotWritten {
        id: ShotId,
        ok: bool,
    },
    /// The manager is being disabled: the link no longer calls it, and every link call gives `LinkError::Gone`.
    ManagerGone,
}

impl ModEvent {
    /// True for events that a full queue must keep: keybind releases (so no bind stays down in the mod) and the lease,
    /// enable, settings, menu entry and language events, and the loader events of a link.
    pub fn is_protected(&self) -> bool {
        matches!(
            self,
            ModEvent::Keybind { phase: KeybindPhase::Released, .. }
                | ModEvent::Enabled
                | ModEvent::Disabled
                | ModEvent::Granted(_)
                | ModEvent::Revoked { .. }
                | ModEvent::Suspended { .. }
                | ModEvent::Resumed(_)
                | ModEvent::CapabilityAvailable(_)
                | ModEvent::SettingsChanged { .. }
                | ModEvent::WorldReloadApply
                | ModEvent::MenuEntryClicked { .. }
                | ModEvent::LocaleChanged
                | ModEvent::InputLeaseSuspended
                | ModEvent::InputLeaseResumed
                | ModEvent::InputLeaseLost
                | ModEvent::FakeKeysReleased { .. }
                | ModEvent::ScreenshotTaken { .. }
                | ModEvent::ScreenshotWritten { .. }
                | ModEvent::ManagerGone
        )
    }
}
