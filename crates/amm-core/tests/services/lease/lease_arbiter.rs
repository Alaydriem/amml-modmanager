use amm_core::config::{ExclusiveResolution, PairKey, ResolutionBook, SuspensionResolution};
use amm_core::services::event::EventHub;
use amm_core::services::lease::LeaseArbiter;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::descriptor::{Capability, Claim, ClaimMode, Descriptor};
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::{ModIndex, ModMask};

const CAMERA: Capability = Capability::Camera;

type Claims = &'static [(Capability, ClaimMode)];

const SHARED_CAMERA: Claims = &[(CAMERA, ClaimMode::Shared)];
const EXCLUSIVE_CAMERA: Claims = &[(CAMERA, ClaimMode::Exclusive)];

struct Rig {
    registry: ModRegistry,
    book: ResolutionBook,
    hub: EventHub,
    arbiter: LeaseArbiter,
}

impl Rig {
    fn new(mods: &[(&str, Claims)]) -> (Rig, Vec<ModIndex>) {
        let mut registry = ModRegistry::new(16);
        let indexes =
            mods.iter().filter_map(|(id, claims)| registry.register(descriptor(id, claims), true).ok()).collect();
        let rig =
            Rig { registry, book: ResolutionBook::new(), hub: EventHub::new(16, 256), arbiter: LeaseArbiter::new() };
        (rig, indexes)
    }

    fn acquire(&mut self, who: ModIndex, capability: Capability) -> AcquireOutcome {
        self.arbiter.acquire(&self.registry, &self.book, who, capability, &mut self.hub)
    }

    fn release(&mut self, who: ModIndex, capability: Capability) {
        self.arbiter.release(who, capability, &mut self.hub);
    }

    fn events(&mut self, who: ModIndex) -> Vec<ModEvent> {
        let queue = self.hub.queue_mut(who);
        std::iter::from_fn(|| queue.pop()).collect()
    }

    fn blocked_by(&self, who: ModIndex, capability: Capability) -> Option<ModIndex> {
        self.arbiter.blocked_by(&self.registry, &self.book, who, capability)
    }
}

fn descriptor(id: &str, claims: Claims) -> Descriptor {
    Descriptor {
        namespace: "test".into(),
        name: id.to_string(),
        display_name: id.to_string(),
        version: "0.1.0".into(),
        author: "test".into(),
        config_path: "config.hcl".into(),
        texture_budget_mb: None,
        texture_budget_count: None,
        claims: claims
            .iter()
            .map(|(capability, mode)| Claim {
                capability: *capability,
                mode: *mode,
                reason: "test".into(),
                hide_vanilla_hud: false,
                block_player_turn: false,
                pass_pointer_motion: false,
                full_block: false,
            })
            .collect(),
        keybinds: Vec::new(),
        input_passthrough: Vec::new(),
        settings: Vec::new(),
    }
}

#[test]
fn shared_acquire_while_exclusive_held_is_denied_with_the_holder() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    assert_eq!(rig.acquire(dejavu, CAMERA), AcquireOutcome::Granted);

    let outcome = rig.acquire(freecam, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Denied { holder: dejavu });
    assert!(!rig.arbiter.holds(freecam, CAMERA));
    assert_eq!(rig.arbiter.exclusive_holder(CAMERA), Some(dejavu));
    assert!(rig.events(freecam).is_empty());
}

#[test]
fn exclusive_acquire_suspends_shared_holders_before_granting() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.acquire(freecam, CAMERA);

    let outcome = rig.acquire(dejavu, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Granted);
    assert!(rig.hub.last_sequence(freecam) < rig.hub.last_sequence(dejavu));
    assert_eq!(
        rig.events(freecam),
        vec![ModEvent::Granted(CAMERA), ModEvent::Suspended { capability: CAMERA, by: dejavu }]
    );
    assert_eq!(rig.events(dejavu), vec![ModEvent::Granted(CAMERA)]);
    assert!(!rig.arbiter.holds(freecam, CAMERA));
    assert_eq!(rig.arbiter.holders(CAMERA).iter().collect::<Vec<_>>(), vec![dejavu]);
}

#[test]
fn releasing_the_exclusive_lease_resumes_suspended_and_notifies_denied_mods() {
    let (mut rig, ids) =
        Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA), ("other", SHARED_CAMERA)]);
    let (dejavu, freecam, other) = (ids[0], ids[1], ids[2]);
    rig.acquire(freecam, CAMERA);
    rig.acquire(dejavu, CAMERA);
    rig.acquire(other, CAMERA);
    rig.events(freecam);

    rig.release(dejavu, CAMERA);

    assert_eq!(rig.events(freecam), vec![ModEvent::Resumed(CAMERA)]);
    assert_eq!(rig.events(other), vec![ModEvent::CapabilityAvailable(CAMERA)]);
    assert_eq!(rig.arbiter.exclusive_holder(CAMERA), None);
    assert!(rig.arbiter.holds(freecam, CAMERA));

    rig.release(freecam, CAMERA);
    rig.acquire(dejavu, CAMERA);
    rig.release(dejavu, CAMERA);
    assert!(rig.events(other).is_empty());
}

#[test]
fn coexist_resolution_neither_suspends_nor_denies() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.book =
        rig.book.with_suspension(PairKey::new(CAMERA, "@test/dejavu", "@test/freecam"), SuspensionResolution::Coexist);
    rig.acquire(freecam, CAMERA);

    assert_eq!(rig.acquire(dejavu, CAMERA), AcquireOutcome::Granted);

    assert_eq!(rig.events(freecam), vec![ModEvent::Granted(CAMERA)]);
    assert_eq!(rig.arbiter.holders(CAMERA).len(), 2);

    rig.release(freecam, CAMERA);
    assert_eq!(rig.acquire(freecam, CAMERA), AcquireOutcome::Granted);
    assert_eq!(rig.blocked_by(freecam, CAMERA), None);
}

#[test]
fn exclusive_clash_deny_second_keeps_the_first_holder() {
    let (mut rig, ids) = Rig::new(&[("alpha", EXCLUSIVE_CAMERA), ("beta", EXCLUSIVE_CAMERA)]);
    let (alpha, beta) = (ids[0], ids[1]);
    rig.acquire(alpha, CAMERA);

    let outcome = rig.acquire(beta, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Denied { holder: alpha });
    assert_eq!(rig.arbiter.exclusive_holder(CAMERA), Some(alpha));
    assert!(rig.events(beta).is_empty());
    assert_eq!(rig.events(alpha), vec![ModEvent::Granted(CAMERA)]);
}

#[test]
fn exclusive_clash_prefer_revokes_the_holder_before_granting() {
    let (mut rig, ids) = Rig::new(&[("alpha", EXCLUSIVE_CAMERA), ("beta", EXCLUSIVE_CAMERA)]);
    let (alpha, beta) = (ids[0], ids[1]);
    rig.book = rig.book.with_exclusive(
        PairKey::new(CAMERA, "@test/alpha", "@test/beta"),
        ExclusiveResolution::Prefer("@test/beta".into()),
    );
    rig.acquire(alpha, CAMERA);

    let outcome = rig.acquire(beta, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Preempted);
    assert_eq!(rig.arbiter.exclusive_holder(CAMERA), Some(beta));
    assert!(!rig.arbiter.holds(alpha, CAMERA));
    assert!(rig.hub.last_sequence(alpha) < rig.hub.last_sequence(beta));
    assert_eq!(rig.events(alpha), vec![ModEvent::Granted(CAMERA), ModEvent::Revoked { capability: CAMERA, by: beta }]);
    assert_eq!(rig.blocked_by(alpha, CAMERA), Some(beta));
}

#[test]
fn prefer_for_the_holder_denies_the_requester() {
    let (mut rig, ids) = Rig::new(&[("alpha", EXCLUSIVE_CAMERA), ("beta", EXCLUSIVE_CAMERA)]);
    let (alpha, beta) = (ids[0], ids[1]);
    rig.book = rig.book.with_exclusive(
        PairKey::new(CAMERA, "@test/alpha", "@test/beta"),
        ExclusiveResolution::Prefer("@test/alpha".into()),
    );
    rig.acquire(alpha, CAMERA);

    assert_eq!(rig.acquire(beta, CAMERA), AcquireOutcome::Denied { holder: alpha });
}

#[test]
fn second_acquire_by_the_exclusive_holder_changes_nothing() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.acquire(freecam, CAMERA);
    rig.acquire(dejavu, CAMERA);
    rig.events(freecam);
    rig.events(dejavu);

    assert_eq!(rig.acquire(dejavu, CAMERA), AcquireOutcome::Granted);

    assert!(rig.events(dejavu).is_empty());
    assert!(rig.events(freecam).is_empty());
    assert!(!rig.arbiter.holds(freecam, CAMERA));
}

#[test]
fn acquire_of_an_unclaimed_capability_is_denied_without_side_effects() {
    let (mut rig, ids) = Rig::new(&[("freecam", SHARED_CAMERA)]);
    let freecam = ids[0];

    let outcome = rig.acquire(freecam, Capability::Overlay);

    assert_eq!(outcome, AcquireOutcome::Denied { holder: freecam });
    assert!(rig.arbiter.holders(Capability::Overlay).is_empty());
    assert_eq!(rig.arbiter.exclusive_holder(Capability::Overlay), None);
    assert!(rig.events(freecam).is_empty());
}

#[test]
fn release_all_releases_every_capability_and_resumes_others() {
    const BOTH_EXCLUSIVE: Claims = &[(CAMERA, ClaimMode::Exclusive), (Capability::Overlay, ClaimMode::Exclusive)];
    const BOTH_SHARED: Claims = &[(CAMERA, ClaimMode::Shared), (Capability::Overlay, ClaimMode::Shared)];
    let (mut rig, ids) = Rig::new(&[("dejavu", BOTH_EXCLUSIVE), ("freecam", BOTH_SHARED), ("other", SHARED_CAMERA)]);
    let (dejavu, freecam, other) = (ids[0], ids[1], ids[2]);
    rig.acquire(freecam, CAMERA);
    rig.acquire(freecam, Capability::Overlay);
    rig.acquire(dejavu, CAMERA);
    rig.acquire(dejavu, Capability::Overlay);
    rig.acquire(other, CAMERA);
    rig.events(freecam);

    rig.arbiter.release_all(dejavu, &mut rig.hub);

    assert_eq!(rig.arbiter.exclusive_holder(CAMERA), None);
    assert_eq!(rig.arbiter.exclusive_holder(Capability::Overlay), None);
    assert!(!rig.arbiter.holds(dejavu, CAMERA));
    assert!(!rig.arbiter.holds(dejavu, Capability::Overlay));
    assert_eq!(rig.events(freecam), vec![ModEvent::Resumed(CAMERA), ModEvent::Resumed(Capability::Overlay)]);
    assert_eq!(rig.events(other), vec![ModEvent::CapabilityAvailable(CAMERA)]);
    assert!(rig.arbiter.holds(freecam, CAMERA));
    assert!(rig.arbiter.holds(freecam, Capability::Overlay));
}

#[test]
fn blocked_by_names_the_holder_without_changing_state() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    assert_eq!(rig.blocked_by(freecam, CAMERA), None);

    rig.acquire(dejavu, CAMERA);

    assert_eq!(rig.blocked_by(freecam, CAMERA), Some(dejavu));
    assert_eq!(rig.blocked_by(dejavu, CAMERA), None);
    assert_eq!(rig.arbiter.holders(CAMERA), ModMask(1 << dejavu.0));
    assert!(rig.events(freecam).is_empty());
}

#[test]
fn most_recent_lists_active_holders_newest_first() {
    let (mut rig, ids) = Rig::new(&[("a", SHARED_CAMERA), ("b", SHARED_CAMERA), ("c", SHARED_CAMERA)]);
    for id in &ids {
        rig.acquire(*id, CAMERA);
    }
    rig.release(ids[1], CAMERA);
    rig.acquire(ids[1], CAMERA);

    let order: Vec<ModIndex> = rig.arbiter.most_recent(CAMERA).collect();

    assert_eq!(order, vec![ids[1], ids[2], ids[0]]);
}

fn assert_blocked_table_agrees(rig: &Rig, ids: &[ModIndex]) {
    for who in ids {
        for capability in Capability::ALL {
            assert_eq!(
                rig.arbiter.blocked(*who, capability),
                rig.blocked_by(*who, capability),
                "mod {} capability {:?}",
                who.0,
                capability
            );
        }
    }
}

#[test]
fn blocked_table_agrees_with_blocked_by_after_every_change() {
    const BOTH_EXCLUSIVE: Claims = &[(CAMERA, ClaimMode::Exclusive), (Capability::Overlay, ClaimMode::Exclusive)];
    const BOTH_SHARED: Claims = &[(CAMERA, ClaimMode::Shared), (Capability::Overlay, ClaimMode::Shared)];
    let (mut rig, ids) = Rig::new(&[
        ("alpha", BOTH_EXCLUSIVE),
        ("beta", BOTH_EXCLUSIVE),
        ("freecam", BOTH_SHARED),
        ("other", SHARED_CAMERA),
    ]);
    let (alpha, beta, freecam, other) = (ids[0], ids[1], ids[2], ids[3]);
    rig.book = rig
        .book
        .with_exclusive(
            PairKey::new(Capability::Overlay, "@test/alpha", "@test/beta"),
            ExclusiveResolution::Prefer("@test/beta".into()),
        )
        .with_suspension(PairKey::new(CAMERA, "@test/beta", "@test/other"), SuspensionResolution::Coexist);
    let steps: Vec<(ModIndex, Capability, bool)> = vec![
        (freecam, CAMERA, true),
        (alpha, CAMERA, true),
        (beta, CAMERA, true),
        (other, CAMERA, true),
        (alpha, Capability::Overlay, true),
        (beta, Capability::Overlay, true),
        (freecam, Capability::Overlay, true),
        (alpha, CAMERA, false),
        (beta, Capability::Overlay, false),
        (beta, CAMERA, true),
        (beta, CAMERA, false),
        (freecam, CAMERA, false),
    ];

    assert_blocked_table_agrees(&rig, &ids);
    for (who, capability, acquire) in steps {
        if acquire {
            rig.acquire(who, capability);
        } else {
            rig.release(who, capability);
        }
        assert_blocked_table_agrees(&rig, &ids);
    }
    rig.arbiter.release_all(beta, &mut rig.hub);
    assert_blocked_table_agrees(&rig, &ids);
    rig.arbiter.release_all(alpha, &mut rig.hub);
    assert_blocked_table_agrees(&rig, &ids);
}

#[test]
fn refresh_blocked_applies_a_new_resolution() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.acquire(dejavu, CAMERA);
    assert_eq!(rig.arbiter.blocked(freecam, CAMERA), Some(dejavu));

    rig.book =
        rig.book.with_suspension(PairKey::new(CAMERA, "@test/dejavu", "@test/freecam"), SuspensionResolution::Coexist);
    assert_eq!(rig.arbiter.blocked(freecam, CAMERA), Some(dejavu));
    rig.arbiter.refresh_blocked(&rig.registry, &rig.book);

    assert_eq!(rig.arbiter.blocked(freecam, CAMERA), None);
    assert_eq!(rig.arbiter.blocked(dejavu, CAMERA), None);
}

#[test]
fn refresh_blocked_drops_a_resolution_that_the_book_no_longer_holds() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.book =
        rig.book.with_suspension(PairKey::new(CAMERA, "@test/dejavu", "@test/freecam"), SuspensionResolution::Coexist);
    rig.acquire(dejavu, CAMERA);
    assert_eq!(rig.arbiter.blocked(freecam, CAMERA), None);

    rig.book = ResolutionBook::new();
    rig.arbiter.refresh_blocked(&rig.registry, &rig.book);

    assert_eq!(rig.arbiter.blocked(freecam, CAMERA), Some(dejavu));
}

#[test]
fn a_suspended_shared_holder_that_acquires_again_is_denied_by_the_exclusive_holder() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_CAMERA), ("freecam", SHARED_CAMERA)]);
    let (dejavu, freecam) = (ids[0], ids[1]);
    rig.acquire(freecam, CAMERA);
    rig.acquire(dejavu, CAMERA);
    rig.events(freecam);

    let outcome = rig.acquire(freecam, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Denied { holder: dejavu });
    assert!(rig.arbiter.is_suspended(freecam, CAMERA));
    assert!(!rig.arbiter.holds(freecam, CAMERA));
    assert!(rig.events(freecam).is_empty());

    rig.release(dejavu, CAMERA);

    let events = rig.events(freecam);
    assert!(events.contains(&ModEvent::Resumed(CAMERA)));
    assert!(!events.contains(&ModEvent::CapabilityAvailable(CAMERA)));
    assert!(rig.arbiter.holds(freecam, CAMERA));
}

#[test]
fn a_holder_that_is_not_suspended_still_gets_granted_on_a_repeat_acquire() {
    let (mut rig, ids) = Rig::new(&[("freecam", SHARED_CAMERA)]);
    let freecam = ids[0];
    rig.acquire(freecam, CAMERA);
    rig.events(freecam);

    let outcome = rig.acquire(freecam, CAMERA);

    assert_eq!(outcome, AcquireOutcome::Granted);
    assert!(rig.arbiter.holds(freecam, CAMERA));
    assert!(rig.events(freecam).is_empty());
}

#[test]
fn held_by_other_is_true_only_when_another_mod_holds_the_capability() {
    let (mut rig, ids) =
        Rig::new(&[("freecam", SHARED_CAMERA), ("minimap", SHARED_CAMERA), ("dejavu", EXCLUSIVE_CAMERA)]);
    let (freecam, minimap, dejavu) = (ids[0], ids[1], ids[2]);
    let empty = rig.arbiter.held_by_other(freecam, CAMERA);
    rig.acquire(freecam, CAMERA);
    let only_self = rig.arbiter.held_by_other(freecam, CAMERA);
    let seen_by_minimap = rig.arbiter.held_by_other(minimap, CAMERA);
    rig.acquire(minimap, CAMERA);
    let both = (rig.arbiter.held_by_other(freecam, CAMERA), rig.arbiter.held_by_other(minimap, CAMERA));
    rig.acquire(dejavu, CAMERA);
    let over_suspended = rig.arbiter.held_by_other(dejavu, CAMERA);
    let under_exclusive = rig.arbiter.held_by_other(freecam, CAMERA);
    rig.release(dejavu, CAMERA);
    rig.release(minimap, CAMERA);
    let after_release = rig.arbiter.held_by_other(freecam, CAMERA);

    assert!(!empty);
    assert!(!only_self);
    assert!(seen_by_minimap);
    assert_eq!(both, (true, true));
    assert!(!over_suspended);
    assert!(under_exclusive);
    assert!(!after_release);
    assert!(!rig.arbiter.held_by_other(freecam, Capability::Hud));
}

const EXCLUSIVE_RENDER_SIZE: Claims = &[(Capability::RenderSize, ClaimMode::Exclusive)];

#[test]
fn a_second_mod_cannot_take_render_size_while_another_holds_it() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_RENDER_SIZE), ("recorder", EXCLUSIVE_RENDER_SIZE)]);
    let (dejavu, recorder) = (ids[0], ids[1]);
    assert_eq!(rig.acquire(dejavu, Capability::RenderSize), AcquireOutcome::Granted);

    assert_eq!(rig.acquire(recorder, Capability::RenderSize), AcquireOutcome::Denied { holder: dejavu });
    assert_eq!(rig.arbiter.exclusive_holder(Capability::RenderSize), Some(dejavu));
}

#[test]
fn a_prefer_resolution_does_not_take_render_size_from_its_holder() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_RENDER_SIZE), ("recorder", EXCLUSIVE_RENDER_SIZE)]);
    let (dejavu, recorder) = (ids[0], ids[1]);
    let key = PairKey::new(Capability::RenderSize, "@test/dejavu", "@test/recorder");
    rig.book = rig.book.with_exclusive(key, ExclusiveResolution::Prefer("@test/recorder".into()));
    rig.acquire(dejavu, Capability::RenderSize);

    assert_eq!(rig.acquire(recorder, Capability::RenderSize), AcquireOutcome::Denied { holder: dejavu });
    assert!(rig.arbiter.holds(dejavu, Capability::RenderSize));
    assert_eq!(rig.blocked_by(recorder, Capability::RenderSize), Some(dejavu));
}

#[test]
fn render_size_is_free_for_the_second_mod_once_the_holder_releases_it() {
    let (mut rig, ids) = Rig::new(&[("dejavu", EXCLUSIVE_RENDER_SIZE), ("recorder", EXCLUSIVE_RENDER_SIZE)]);
    let (dejavu, recorder) = (ids[0], ids[1]);
    rig.acquire(dejavu, Capability::RenderSize);
    rig.acquire(recorder, Capability::RenderSize);

    rig.release(dejavu, Capability::RenderSize);

    assert_eq!(rig.events(recorder), vec![ModEvent::CapabilityAvailable(Capability::RenderSize)]);
    assert_eq!(rig.acquire(recorder, Capability::RenderSize), AcquireOutcome::Granted);
}
