use amm_abi::types::{AmmCallbacks, AmmEvent, AmmEventKind};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::lease::AcquireOutcome;

use crate::support::{Descriptors, HostFixture};

const NO_CALLBACKS: AmmCallbacks = AmmCallbacks {
    size: size_of::<AmmCallbacks>() as u32,
    user: std::ptr::null_mut(),
    frame: None,
    textures_lost: None,
};
const KEY_L: u32 = 0x4C;
const EVENT_ROOM: usize = 16;

#[test]
fn a_mod_registered_without_a_frame_callback_gets_events_and_draws_no_layer() {
    let fixture = HostFixture::new();
    let bind = Descriptors::chord_bind("pulse", "WorldDraw", "Ctrl+L");
    let descriptor = Descriptors::with_claims("worldly", &[("WorldDraw", "Shared")], &bind);
    let dir = fixture.next_mod_dir();
    let handle = fixture
        .with_runtime(|runtime| runtime.register(descriptor.as_bytes(), &dir, NO_CALLBACKS))
        .unwrap_or_else(|status| panic!("register: {status:?}"));

    let outcome = fixture
        .with_runtime(|runtime| runtime.acquire(handle, Capability::WorldDraw))
        .unwrap_or_else(|status| panic!("acquire: {status:?}"));
    fixture.key(KEY_L, true, HostFixture::CTRL, 100);
    fixture.key(KEY_L, false, HostFixture::CTRL, 110);
    let frames: Vec<Vec<u8>> = (0..4).map(|now| HostFixture::owners(&fixture.frame(200 + now))).collect();
    let mut events = Vec::new();
    let polled = fixture
        .with_runtime(|runtime| runtime.poll_events(handle, EVENT_ROOM, |_, event: AmmEvent| events.push(event)));

    assert!(matches!(outcome, AcquireOutcome::Granted));
    assert!(polled.is_ok());
    assert!(events.iter().any(|event| event.kind == AmmEventKind::Keybind && event.phase == 0));
    assert!(frames.iter().all(|owners| !owners.contains(&(handle.slot as u8))));
}
