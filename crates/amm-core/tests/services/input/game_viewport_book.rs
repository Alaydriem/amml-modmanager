use amm_core::config::ResolutionBook;
use amm_core::services::descriptor::DescriptorChecker;
use amm_core::services::event::EventHub;
use amm_core::services::input::GameViewportBook;
use amm_core::services::lease::LeaseArbiter;
use amm_core::services::registry::ModRegistry;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::input::GameViewport;
use amm_core::structs::registry::{ModIndex, ModMask};

const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");

struct Rig {
    registry: ModRegistry,
    book: ResolutionBook,
    arbiter: LeaseArbiter,
    hub: EventHub,
    viewports: GameViewportBook,
    dejavu: ModIndex,
}

impl Rig {
    fn new() -> Rig {
        let mut registry = ModRegistry::new(16);
        let descriptor = DescriptorChecker::new(128, 256).parse(DEJAVU).unwrap_or_else(|error| panic!("{error}"));
        let dejavu = registry.register(descriptor, true).unwrap_or_else(|error| panic!("{error:?}"));
        let book = ResolutionBook::new();
        let mut arbiter = LeaseArbiter::new();
        arbiter.refresh_blocked(&registry, &book);
        Rig { registry, book, arbiter, hub: EventHub::new(16, 256), viewports: GameViewportBook::new(), dejavu }
    }

    fn hold_overlay(&mut self) {
        self.arbiter.acquire(&self.registry, &self.book, self.dejavu, Capability::Overlay, &mut self.hub);
    }

    fn viewport() -> GameViewport {
        GameViewport::new([100.0, 100.0, 500.0, 400.0]).unwrap_or_else(|| panic!("a valid rect"))
    }
}

#[test]
fn the_viewport_of_the_overlay_holder_is_found_under_the_pointer() {
    let mut rig = Rig::new();
    rig.hold_overlay();
    rig.viewports.set(rig.dejavu, Some(Rig::viewport()));

    assert_eq!(rig.viewports.at((200.0, 200.0), &rig.arbiter), Some((rig.dejavu, Rig::viewport())));
    assert_eq!(rig.viewports.at((600.0, 200.0), &rig.arbiter), None);
}

#[test]
fn a_mod_that_does_not_hold_overlay_has_no_viewport() {
    let mut rig = Rig::new();
    rig.viewports.set(rig.dejavu, Some(Rig::viewport()));

    assert_eq!(rig.viewports.at((200.0, 200.0), &rig.arbiter), None);
}

#[test]
fn a_cleared_viewport_is_gone() {
    let mut rig = Rig::new();
    rig.hold_overlay();
    rig.viewports.set(rig.dejavu, Some(Rig::viewport()));

    rig.viewports.set(rig.dejavu, None);

    assert_eq!(rig.viewports.at((200.0, 200.0), &rig.arbiter), None);
}

#[test]
fn the_book_holds_one_viewport_per_slot_and_ignores_a_slot_past_the_capacity() {
    let mut rig = Rig::new();
    rig.hold_overlay();
    let past = ModIndex(ModMask::CAPACITY as u8);

    rig.viewports.set(past, Some(Rig::viewport()));
    rig.viewports.set(rig.dejavu, Some(Rig::viewport()));
    rig.viewports.set(rig.dejavu, Some(GameViewport::new([0.0, 0.0, 50.0, 50.0]).unwrap_or_else(|| panic!("rect"))));

    assert_eq!(rig.viewports.at((200.0, 200.0), &rig.arbiter), None, "a second set replaces the first");
    assert!(rig.viewports.at((10.0, 10.0), &rig.arbiter).is_some());
}
