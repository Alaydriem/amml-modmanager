use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;

use crate::support::{Descriptors, HostFixture, SimMod};

const RESOLUTIONS: &str = "resolutions {\n  exclusive = {\n    Input__test_alpha__test_beta = \"Prefer:@test/alpha\"\n    \
                           Overlay__test_alpha__test_beta = \"Prefer:@test/beta\"\n  }\n}\n";
const CLAIMS: [(&str, &str); 3] = [("Camera", "Exclusive"), ("Input", "Exclusive"), ("Overlay", "Exclusive")];

#[test]
fn each_exclusive_resolution_gives_its_outcome_and_the_loser_is_revoked() {
    let fixture = HostFixture::with_config(RESOLUTIONS);
    let mut alpha = SimMod::new(&fixture, Descriptors::with_claims("alpha", &CLAIMS, ""), "");
    let mut beta = SimMod::new(&fixture, Descriptors::with_claims("beta", &CLAIMS, ""), "");
    alpha.drain();
    beta.drain();

    let counts = fixture.with_runtime(|runtime| {
        let report = runtime.manager().clash_report();
        (
            report.conflicts.len(),
            report.conflict_count(),
            report.badge_count(ModIndex(0)),
            report.badge_count(ModIndex(1)),
        )
    });
    assert_eq!(counts, (3, 1, 1, 1));

    assert_eq!(beta.acquire(Capability::Camera), AcquireOutcome::Granted);
    let deny_second = alpha.acquire(Capability::Camera);
    assert_eq!(deny_second, AcquireOutcome::Denied { holder: ModIndex(1) });
    assert_eq!(alpha.holder(Capability::Camera).as_deref(), Some("@test/beta"));
    assert!(beta.drain().iter().all(|event| !matches!(event, ModEvent::Revoked { .. })));

    assert_eq!(beta.acquire(Capability::Input), AcquireOutcome::Granted);
    beta.drain();
    alpha.drain();
    let prefer_alpha = alpha.acquire(Capability::Input);
    let revoked = beta.drain();
    let granted = alpha.drain();
    assert_eq!(prefer_alpha, AcquireOutcome::Preempted);
    assert_eq!(revoked, vec![ModEvent::Revoked { capability: Capability::Input, by: ModIndex(0) }]);
    assert_eq!(granted, vec![ModEvent::Granted(Capability::Input)]);
    assert_eq!(beta.holder(Capability::Input).as_deref(), Some("@test/alpha"));

    assert_eq!(alpha.acquire(Capability::Overlay), AcquireOutcome::Granted);
    alpha.drain();
    beta.drain();
    let prefer_beta = beta.acquire(Capability::Overlay);
    let revoked = alpha.drain();
    let granted = beta.drain();
    assert_eq!(prefer_beta, AcquireOutcome::Preempted);
    assert_eq!(revoked, vec![ModEvent::Revoked { capability: Capability::Overlay, by: ModIndex(1) }]);
    assert_eq!(granted, vec![ModEvent::Granted(Capability::Overlay)]);
    assert_eq!(alpha.holder(Capability::Overlay).as_deref(), Some("@test/beta"));
}
