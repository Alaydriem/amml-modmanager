use amm_abi::types::AmmStatus;
use amm_abi::types::{AmmEvent, AmmEventKind};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent, ReleaseReason};
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
    ];
    for capability in Capability::MANAGER {
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
        let Ok(abi) = AmmEvent::try_from(event) else { panic!("{event:?} has no ABI kind") };
        assert_eq!(ModEvent::try_from(&abi), Ok(event), "{event:?}");
    }
}

#[test]
fn an_input_capability_is_written_as_a_byte_no_reader_takes() {
    let Ok(abi) = AmmEvent::try_from(ModEvent::Granted(Capability::Input)) else { panic!("no ABI kind") };

    assert_eq!(abi.kind, AmmEventKind::Granted);
    assert!(ModEvent::try_from(&abi).is_err());
}

#[test]
fn an_unknown_capability_byte_is_rejected() {
    let Ok(mut abi) = AmmEvent::try_from(ModEvent::Granted(Capability::Camera)) else { panic!("no ABI kind") };
    abi.capability = 200;
    assert!(ModEvent::try_from(&abi).is_err());
}

#[test]
fn an_unknown_phase_byte_is_rejected() {
    let Ok(mut abi) = AmmEvent::try_from(ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed }) else {
        panic!("no ABI kind")
    };
    abi.phase = 99;
    assert!(ModEvent::try_from(&abi).is_err());
}

#[test]
fn the_loader_events_of_a_link_have_no_abi_kind() {
    let events = [
        ModEvent::InputLeaseSuspended,
        ModEvent::InputLeaseResumed,
        ModEvent::InputLeaseLost,
        ModEvent::FakeKeysReleased { reason: ReleaseReason::FocusLost },
        ModEvent::ScreenshotTaken { id: amml_api::ShotId(1), ok: true },
        ModEvent::ScreenshotWritten { id: amml_api::ShotId(1), ok: false },
        ModEvent::ManagerGone,
    ];

    for event in events {
        assert_eq!(AmmEvent::try_from(event).err(), Some(AmmStatus::InvalidArgument), "{event:?}");
    }
}
