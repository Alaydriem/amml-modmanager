use amm_core::config::ManagerConfig;
use amm_core::services::ManagerService;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::input::{InputEvent, Modifiers, RouteDecision};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;

/// A manager with one mod that holds Input exclusively, with or without `full_block`. It measures the router
/// on the holder path.
pub struct HolderRoute {
    manager: ManagerService,
    holder: ModIndex,
    toggles: u64,
}

impl HolderRoute {
    pub const VK_Q: u32 = 0x51;
    const DRAIN_EVERY: u64 = 64;
    const EVENT_BATCH: usize = 64;

    pub fn new(full_block: bool) -> Self {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let text = format!(
            "namespace = \"bench\"\nname = \"holder\"\ndisplay_name = \"Holder\"\nversion = \"0.1.0\"\n\
             author = \"bench\"\nconfig_path = \"config.hcl\"\ninput_passthrough = [81]\n\
             claim \"Input\" {{\n  mode = \"Exclusive\"\n  reason = \"bench\"\n  full_block = {full_block}\n  \
             pass_pointer_motion = true\n}}\n"
        );
        let dir = std::env::temp_dir().join("amm-bench").join("holder");
        let registered = manager.register(&text, None, &dir);
        let holder = registered.unwrap_or_else(|error| panic!("holder does not register: {error}")).0;
        let outcome = manager.acquire(holder, Capability::Input, 0);
        assert!(matches!(outcome, Ok(AcquireOutcome::Granted)), "holder does not get Input: {outcome:?}");
        let mut fixture = Self { manager, holder, toggles: 0 };
        fixture.drain_events();
        // Q is in the passthrough list and motion is passed on, so only full_block keeps them from the game
        let key = fixture.route_key(Self::VK_Q, true);
        let motion = fixture.route_motion();
        fixture.route_key(Self::VK_Q, false);
        assert_eq!(key.consume_from_game, full_block, "holder does not block as expected");
        assert_eq!(motion.consume_from_game, full_block, "holder does not block as expected");
        fixture
    }

    /// One key event through the router, then a periodic drain of the event queue.
    pub fn route_key(&mut self, vk: u32, down: bool) -> RouteDecision {
        let event =
            InputEvent::Key { vk, down, repeat: false, mods: Modifiers { ctrl: false, shift: false, alt: false } };
        let decision = self.manager.route(&event, 0);
        self.toggles += 1;
        if self.toggles.is_multiple_of(Self::DRAIN_EVERY) {
            self.drain_events();
        }
        decision
    }

    /// One pointer motion event through the router.
    pub fn route_motion(&mut self) -> RouteDecision {
        let decision = self.manager.route(&InputEvent::PointerDelta { dx: 1.0, dy: 1.0 }, 0);
        self.toggles += 1;
        if self.toggles.is_multiple_of(Self::DRAIN_EVERY) {
            self.drain_events();
        }
        decision
    }

    fn drain_events(&mut self) {
        let mut batch = [ModEvent::Enabled; Self::EVENT_BATCH];
        while self.manager.poll_events(self.holder, &mut batch) == batch.len() {}
    }
}
