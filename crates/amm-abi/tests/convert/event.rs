use amm_abi::types::{AmmEvent, AmmEventKind};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::registry::ModIndex;

#[test]
fn every_event_kind_survives_core_to_abi_to_core() {
    let by = ModIndex(7);
    let mut events = vec![
        ModEvent::Enabled,
        ModEvent::Disabled,
        ModEvent::SettingsChanged { paths: 0xDEAD_BEEF },
        ModEvent::WorldReloadApply,
        ModEvent::MenuEntryClicked { entry: 2 },
        ModEvent::LocaleChanged,
        ModEvent::PointerButton { button: 2, down: true, x: -12.5, y: 400.25 },
        ModEvent::PointerButton { button: 0, down: false, x: 0.0, y: 1.0 },
        ModEvent::PointerDelta { dx: -3.0, dy: 4.5 },
        ModEvent::Wheel { delta: -1.5 },
        ModEvent::Key { vk: 0x4D, down: true, repeat: false },
        ModEvent::Key { vk: 0x4D, down: true, repeat: true },
        ModEvent::Key { vk: 0xFF, down: false, repeat: false },
    ];
    for capability in Capability::ALL {
        events.push(ModEvent::Granted(capability));
        events.push(ModEvent::Revoked { capability, by });
        events.push(ModEvent::Suspended { capability, by });
        events.push(ModEvent::Resumed(capability));
        events.push(ModEvent::CapabilityAvailable(capability));
    }
    for phase in [KeybindPhase::Pressed, KeybindPhase::Released, KeybindPhase::Repeat] {
        events.push(ModEvent::Keybind { action: u16::MAX, phase });
    }

    for event in events {
        let abi = AmmEvent::from(event);
        assert_eq!(ModEvent::try_from(&abi), Ok(event), "{event:?}");
    }
}

#[test]
fn fake_keys_released_round_trips() {
    let abi = AmmEvent::from(ModEvent::FakeKeysReleased);

    assert_eq!(abi.kind, AmmEventKind::FakeKeysReleased);
    assert_eq!(ModEvent::try_from(&abi), Ok(ModEvent::FakeKeysReleased));
}

#[test]
fn an_unknown_capability_byte_is_rejected() {
    let mut abi = AmmEvent::from(ModEvent::Granted(Capability::Camera));
    abi.capability = 200;
    assert!(ModEvent::try_from(&abi).is_err());
}

#[test]
fn an_unknown_phase_byte_is_rejected() {
    let mut abi = AmmEvent::from(ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed });
    abi.phase = 99;
    assert!(ModEvent::try_from(&abi).is_err());
}
