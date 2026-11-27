use amm_client::link::LinkState;
use amm_client::test_support::FakeTable;
use amm_core::errors::LinkError;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::lease::AcquireOutcome;

use crate::support::{Descriptors, HostFixture, SimMod};

#[test]
fn a_mod_reaches_the_manager_through_the_registry_and_sees_it_go() {
    let fixture = HostFixture::new();
    let claims = [("Overlay", "Shared")];
    let mut sim = SimMod::new(&fixture, Descriptors::with_claims("sim-reg", &claims, ""), "");
    let connected = sim.link().state();
    let outcome = sim.acquire(Capability::Overlay);
    let owners_before = HostFixture::owners(&fixture.frame(100));

    FakeTable::gone(fixture.loader());
    let after_gone = sim.link().acquire(Capability::Overlay);
    let owners_after = HostFixture::owners(&fixture.frame(200));
    let events = sim.drain();

    assert_eq!(connected, LinkState::Connected);
    assert!(matches!(outcome, AcquireOutcome::Granted));
    assert_eq!(owners_before.len(), 1, "the mod's layer is drawn while it is linked");
    assert_eq!(sim.link().state(), LinkState::Gone);
    assert_eq!(after_gone, Err(LinkError::Gone));
    assert!(owners_after.is_empty(), "the manager took the mod's layer away when the link let go");
    assert!(events.contains(&amm_core::structs::event::ModEvent::ManagerGone));
}
