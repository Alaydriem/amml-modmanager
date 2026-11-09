use crate::structs::descriptor::Capability;
use crate::structs::event::KeybindPhase;
use crate::structs::registry::ModIndex;

/// One event for one mod. Plain data, so a queue of them is a flat array.
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
    Key {
        vk: u32,
        down: bool,
        repeat: bool,
    },
    PointerButton {
        button: u8,
        down: bool,
        x: f32,
        y: f32,
    },
    PointerDelta {
        dx: f32,
        dy: f32,
    },
    Wheel {
        delta: f32,
    },
    /// The pixels of capture `sequence` are in the manager's buffer, or the copy failed or was cancelled.
    ScreenshotTaken {
        sequence: u32,
        ok: bool,
    },
    /// The file of capture `sequence` is in place, or the encode or the write failed.
    ScreenshotWritten {
        sequence: u32,
        ok: bool,
    },
    /// The person clicked the mod's menu entry `entry` in the corner row.
    MenuEntryClicked {
        entry: u16,
    },
    /// The person picked another language in the manager. Only a mod that has read the language code gets it;
    /// the mod reads the code again.
    LocaleChanged,
    /// The manager let go of the fake keys or mouse buttons the mod held: the panel opened, the window lost focus,
    /// the world was left or the mod was disabled. The mod drops its own record of them.
    FakeKeysReleased,
}

impl ModEvent {
    /// True for events that a full queue must keep: key-ups, keybind releases, button-ups (so nothing stays
    /// down in the mod) and the lease, enable, settings, screenshot, menu entry, language and fake-key release events.
    pub fn is_protected(&self) -> bool {
        matches!(
            self,
            ModEvent::Key { down: false, .. }
                | ModEvent::Keybind { phase: KeybindPhase::Released, .. }
                | ModEvent::PointerButton { down: false, .. }
                | ModEvent::Enabled
                | ModEvent::Disabled
                | ModEvent::Granted(_)
                | ModEvent::Revoked { .. }
                | ModEvent::Suspended { .. }
                | ModEvent::Resumed(_)
                | ModEvent::CapabilityAvailable(_)
                | ModEvent::SettingsChanged { .. }
                | ModEvent::WorldReloadApply
                | ModEvent::ScreenshotTaken { .. }
                | ModEvent::ScreenshotWritten { .. }
                | ModEvent::MenuEntryClicked { .. }
                | ModEvent::LocaleChanged
                | ModEvent::FakeKeysReleased
        )
    }
}
