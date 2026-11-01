use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;

use crate::support::{Descriptors, HostFixture, SimMod};

const COEXIST: &str = "resolutions {\n  suspension = { WorldDraw__test_cozy__test_hog = \"Coexist\" }\n}\n";

#[test]
fn an_exclusive_holder_suspends_a_shared_one_which_resumes_on_release_and_coexist_does_not_suspend() {
    let fixture = HostFixture::with_config(COEXIST);
    let mut freecam = SimMod::new(&fixture, Descriptors::FREECAM, "");
    let mut dejavu = SimMod::new(&fixture, Descriptors::DEJAVU, "");
    let mut cozy = SimMod::new(&fixture, Descriptors::with_claims("cozy", &[("WorldDraw", "Shared")], ""), "");
    let mut hog = SimMod::new(&fixture, Descriptors::with_claims("hog", &[("WorldDraw", "Exclusive")], ""), "");
    for sim in [&mut freecam, &mut dejavu, &mut cozy, &mut hog] {
        sim.drain();
    }

    assert_eq!(freecam.acquire(Capability::Camera), AcquireOutcome::Granted);
    freecam.drain();
    let taken = dejavu.acquire(Capability::Camera);
    let suspended = freecam.drain();
    let granted = dejavu.drain();
    dejavu.release(Capability::Camera);
    let resumed = freecam.drain();

    assert_eq!(taken, AcquireOutcome::Granted);
    assert_eq!(suspended, vec![ModEvent::Suspended { capability: Capability::Camera, by: ModIndex(1) }]);
    assert_eq!(granted, vec![ModEvent::Granted(Capability::Camera)]);
    assert_eq!(resumed, vec![ModEvent::Resumed(Capability::Camera)]);

    assert_eq!(cozy.acquire(Capability::WorldDraw), AcquireOutcome::Granted);
    cozy.drain();
    assert_eq!(hog.acquire(Capability::WorldDraw), AcquireOutcome::Granted);
    hog.release(Capability::WorldDraw);
    let cozy_events = cozy.drain();

    assert_eq!(cozy_events, Vec::<ModEvent>::new());
}
