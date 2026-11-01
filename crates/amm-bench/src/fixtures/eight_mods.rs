use std::sync::Arc;

use amm_core::config::ManagerConfig;
use amm_core::services::ManagerService;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::input::{InputEvent, Modifiers, RouteDecision};
use amm_core::structs::registry::ModIndex;
use amm_core::structs::snapshot::RenderSnapshot;

use super::Descriptors;

/// A manager with eight mods registered: the four recorded descriptors plus four synthetic ones, each holding the
/// leases it can get.
pub struct EightMods {
    manager: ManagerService,
    mods: Vec<ModIndex>,
    toggles: u64,
    panel_open: bool,
}

impl EightMods {
    pub const VK_B: u32 = 0x42;
    pub const VK_Q: u32 = 0x51;
    const SYNTHETIC: usize = 4;
    const BINDS_PER_SYNTHETIC: usize = 6;
    const DRAIN_EVERY: u64 = 64;
    const EVENT_BATCH: usize = 64;

    pub fn new() -> Self {
        let mut manager = ManagerService::new(ManagerConfig::default(), None);
        let mut texts: Vec<(String, String)> = [
            ("freecam", Descriptors::FREECAM),
            ("dejavu", Descriptors::DEJAVU),
            ("levels", Descriptors::LEVELS),
            ("minimap", Descriptors::MINIMAP),
        ]
        .into_iter()
        .map(|(id, text)| (id.to_owned(), text.to_owned()))
        .collect();
        texts.extend((0..Self::SYNTHETIC).map(|n| (format!("synth{n}"), Self::synthetic(n))));
        let mut mods = Vec::with_capacity(texts.len());
        for (id, text) in &texts {
            let dir = std::env::temp_dir().join("amm-bench").join(id);
            let registered = manager.register(text, None, &dir);
            mods.push(registered.unwrap_or_else(|error| panic!("{id} does not register: {error}")).0);
        }
        let mut fixture = Self { manager, mods, toggles: 0, panel_open: false };
        fixture.acquire_all();
        fixture
    }

    pub fn mod_count(&self) -> usize {
        self.mods.len()
    }

    /// One key event through the router, with Ctrl held. The event queues are drained every few calls, so a
    /// queue does not sit full and drop.
    pub fn route_key(&mut self, vk: u32, down: bool, now_ms: u64) -> RouteDecision {
        let event =
            InputEvent::Key { vk, down, repeat: false, mods: Modifiers { ctrl: true, shift: false, alt: false } };
        let decision = self.manager.route(&event, now_ms);
        self.toggles += 1;
        if self.toggles.is_multiple_of(Self::DRAIN_EVERY) {
            self.drain_events();
        }
        decision
    }

    /// One lease change that republishes the snapshot, then the snapshot read.
    pub fn change_then_snapshot(&mut self, now_ms: u64) -> Arc<RenderSnapshot> {
        let index = self.mods[Self::SYNTHETIC];
        if self.toggles.is_multiple_of(2) {
            self.manager.release(index, Capability::Overlay);
        } else {
            let _ = self.manager.acquire(index, Capability::Overlay, now_ms);
        }
        self.toggles += 1;
        if self.toggles.is_multiple_of(Self::DRAIN_EVERY) {
            self.drain_events();
        }
        self.manager.snapshot()
    }

    /// One panel open or close, which republishes the snapshot without touching a lease, then the snapshot read.
    pub fn publish_then_snapshot(&mut self) -> Arc<RenderSnapshot> {
        self.panel_open = !self.panel_open;
        self.manager.set_panel_open(self.panel_open);
        self.manager.snapshot()
    }

    /// The snapshot read alone.
    pub fn snapshot(&self) -> Arc<RenderSnapshot> {
        self.manager.snapshot()
    }

    // binds on F7 and up, because F3 is a modifier and never a chord's key
    fn synthetic(n: usize) -> String {
        let modifiers = ["Ctrl", "Shift", "Alt", "Ctrl+Shift"][n % 4];
        let chords: Vec<String> =
            (0..Self::BINDS_PER_SYNTHETIC).map(|bind| format!("{modifiers}+F{}", bind + 7)).collect();
        Descriptors::with_binds(&format!("synth{n}"), &chords)
    }

    fn acquire_all(&mut self) {
        const WANTED: [Capability; 4] =
            [Capability::Camera, Capability::Input, Capability::Overlay, Capability::WorldDraw];
        for index in self.mods.clone() {
            for capability in WANTED {
                let _ = self.manager.acquire(index, capability, 0);
            }
        }
        self.drain_events();
    }

    fn drain_events(&mut self) {
        let mut batch = [ModEvent::Enabled; Self::EVENT_BATCH];
        for index in self.mods.clone() {
            while self.manager.poll_events(index, &mut batch) == batch.len() {}
        }
    }
}

impl Default for EightMods {
    fn default() -> Self {
        Self::new()
    }
}
