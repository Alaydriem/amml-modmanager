use amm_core::structs::descriptor::Capability;

use crate::support::{Descriptors, HostFixture, SimBehavior, SimMod};

const FAILURE_LIMIT: u32 = 30;
const FRAMES: u64 = 40;
const BROKEN_SLOT: u8 = 0;
const STEADY_SLOT: u8 = 1;

#[test]
fn a_panicking_frame_callback_is_disabled_after_thirty_failures_and_the_other_mod_still_draws() {
    let fixture = HostFixture::new();
    let claims = [("Overlay", "Shared")];
    let broken =
        SimMod::with_behavior(&fixture, Descriptors::with_claims("broken", &claims, ""), "", SimBehavior::Panic);
    let steady = SimMod::new(&fixture, Descriptors::with_claims("steady", &claims, ""), "");
    broken.acquire(Capability::Overlay);
    steady.acquire(Capability::Overlay);

    let owners: Vec<Vec<u8>> = (0..FRAMES).map(|now| HostFixture::owners(&fixture.frame(now))).collect();

    assert_eq!(broken.frames(), FAILURE_LIMIT);
    assert_eq!(steady.frames(), FRAMES as u32);
    assert!(owners.iter().all(|frame| frame.contains(&STEADY_SLOT) && !frame.contains(&BROKEN_SLOT)));
    assert!(owners[FAILURE_LIMIT as usize].contains(&HostFixture::MANAGER));
    assert_eq!(steady.holder(Capability::Overlay).as_deref(), Some("@test/steady"));
}
