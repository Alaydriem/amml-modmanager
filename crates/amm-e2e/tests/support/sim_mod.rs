use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_client::link::{ForwardSink, ManagerLink};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;

use super::{HostFixture, SimBehavior, SimHandler};

/// A client mod connected through `amm-client`, with its own config folder under the fixture.
pub struct SimMod {
    link: ManagerLink,
    frames: Arc<AtomicU32>,
    config_dir: PathBuf,
}

impl SimMod {
    /// Registers a mod that draws a rectangle. `config_hcl` is written to the mod's `config.hcl` first, unless it is
    /// empty.
    pub fn new(fixture: &HostFixture, descriptor_hcl: &'static str, config_hcl: &str) -> SimMod {
        Self::with_behavior(fixture, descriptor_hcl, config_hcl, SimBehavior::Draw)
    }

    pub fn with_behavior(
        fixture: &HostFixture,
        descriptor_hcl: &'static str,
        config_hcl: &str,
        behavior: SimBehavior,
    ) -> SimMod {
        let config_dir = fixture.next_mod_dir();
        fs::create_dir_all(&config_dir).unwrap_or_else(|error| panic!("mod config dir: {error}"));
        if !config_hcl.is_empty() {
            fs::write(config_dir.join("config.hcl"), config_hcl).unwrap_or_else(|error| panic!("mod config: {error}"));
        }
        let frames = Arc::new(AtomicU32::new(0));
        let handler = SimHandler::new(behavior, Arc::clone(&frames));
        let link = ManagerLink::new(fixture.table(), descriptor_hcl, &config_dir, handler)
            .unwrap_or_else(|error| panic!("register: {error:?}"));
        Self { link, frames, config_dir }
    }

    pub fn acquire(&self, capability: Capability) -> AcquireOutcome {
        self.link.acquire(capability).unwrap_or_else(|error| panic!("acquire: {error:?}"))
    }

    pub fn release(&self, capability: Capability) {
        self.link.release(capability);
    }

    pub fn holder(&self, capability: Capability) -> Option<String> {
        self.link.holder(capability)
    }

    pub fn link(&self) -> &ManagerLink {
        &self.link
    }

    pub fn sink(&self) -> ForwardSink {
        self.link.sink()
    }

    /// Every event queued for the mod, oldest first.
    pub fn drain(&mut self) -> Vec<ModEvent> {
        let mut all = Vec::new();
        loop {
            let batch: Vec<ModEvent> = self.link.events().collect();
            if batch.is_empty() {
                return all;
            }
            all.extend(batch);
        }
    }

    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::SeqCst)
    }

    pub fn config_path(&self) -> PathBuf {
        self.config_dir.join("config.hcl")
    }
}
