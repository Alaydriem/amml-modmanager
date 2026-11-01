use crate::services::lease::LeaseArbiter;
use crate::structs::descriptor::Capability;
use crate::structs::input::GameViewport;
use crate::structs::registry::{ModIndex, ModMask};

/// The game viewport each mod reported, one per mod slot: a new one replaces the old, and a slot past the
/// capacity is ignored. Only the viewport of a mod that holds `Overlay` counts, so a lost lease needs no clean-up
/// here; an unregister clears the slot.
pub struct GameViewportBook {
    viewports: [Option<GameViewport>; ModMask::CAPACITY],
}

impl GameViewportBook {
    pub fn new() -> Self {
        Self { viewports: [None; ModMask::CAPACITY] }
    }

    pub fn set(&mut self, index: ModIndex, viewport: Option<GameViewport>) {
        if let Some(slot) = self.viewports.get_mut(usize::from(index.0)) {
            *slot = viewport;
        }
    }

    /// The viewport under `point` of a mod that holds `Overlay`, with that mod.
    pub fn at(&self, point: (f32, f32), arbiter: &LeaseArbiter) -> Option<(ModIndex, GameViewport)> {
        self.viewports.iter().enumerate().find_map(|(slot, viewport)| {
            let viewport = (*viewport)?;
            let index = ModIndex(u8::try_from(slot).ok()?);
            (viewport.contains(point) && arbiter.holds(index, Capability::Overlay)).then_some((index, viewport))
        })
    }
}

impl Default for GameViewportBook {
    fn default() -> Self {
        Self::new()
    }
}
