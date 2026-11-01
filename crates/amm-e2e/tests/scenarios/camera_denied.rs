use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::toast::ToastKind;

use crate::support::{Descriptors, HostFixture, SimMod};

const KEY_P: u32 = 0x50;

fn blocked_toasts(fixture: &HostFixture) -> usize {
    fixture.toasts().iter().filter(|kind| matches!(kind, ToastKind::Blocked { .. })).count()
}

#[test]
fn a_denied_camera_blocks_the_keybind_with_a_toast_until_the_holder_releases() {
    let fixture = HostFixture::new();
    let mut freecam = SimMod::new(&fixture, Descriptors::FREECAM, "");
    let mut dejavu = SimMod::new(&fixture, Descriptors::DEJAVU, "");
    freecam.drain();
    dejavu.drain();

    let held = dejavu.acquire(Capability::Camera);
    let denied = freecam.acquire(Capability::Camera);
    let holder = freecam.holder(Capability::Camera);
    let blocked_before = blocked_toasts(&fixture);
    let pressed = fixture.key(KEY_P, true, HostFixture::CTRL, 3000);
    let released = fixture.key(KEY_P, false, HostFixture::CTRL, 3010);
    let blocked_after = blocked_toasts(&fixture);
    let frame = fixture.frame(3020);
    let blocked_events = freecam.drain();

    assert_eq!(held, AcquireOutcome::Granted);
    assert!(matches!(denied, AcquireOutcome::Denied { .. }));
    assert_eq!(holder.as_deref(), Some("@alaydriem/dejavu"));
    assert!(pressed.consume && released.consume);
    assert!(blocked_events.iter().all(|event| !matches!(event, ModEvent::Keybind { .. })));
    assert_eq!((blocked_before, blocked_after), (0, 1));
    assert_eq!(HostFixture::owners(&frame), vec![HostFixture::MANAGER]);

    dejavu.release(Capability::Camera);
    let available = freecam.drain();
    fixture.key(KEY_P, true, HostFixture::CTRL, 6000);
    fixture.key(KEY_P, false, HostFixture::CTRL, 6010);
    let delivered = freecam.drain();

    assert!(available.contains(&ModEvent::CapabilityAvailable(Capability::Camera)));
    assert!(delivered.contains(&ModEvent::Keybind { action: 0, phase: KeybindPhase::Pressed }));
}
