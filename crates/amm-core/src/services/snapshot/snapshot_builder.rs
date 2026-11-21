use crate::services::lease::LeaseArbiter;
use crate::services::registry::ModRegistry;
use crate::structs::descriptor::Capability;
use crate::structs::frame::{FrameInputData, LayerKind};
use crate::structs::registry::ModIndex;
use crate::structs::snapshot::LayerEntry;

/// Derives the drawn parts of a `RenderSnapshot` from the registry and the leases: the layer order
/// and whether the vanilla HUD is hidden.
pub struct SnapshotBuilder;

impl SnapshotBuilder {
    /// The layers bottom to top, one per (mod, kind) the mod holds and is not suspended on: Hud
    /// layers in registration order, then Overlay layers in registration order. `registered_seq` gives
    /// the registration order of a mod.
    pub fn layers(
        registry: &ModRegistry,
        arbiter: &LeaseArbiter,
        registered_seq: impl Fn(ModIndex) -> u64,
    ) -> Vec<LayerEntry> {
        let mut drawn: Vec<(u64, ModIndex)> = registry
            .iter()
            .filter(|registered| registered.enabled && !registered.failed)
            .map(|registered| (registered_seq(registered.index), registered.index))
            .collect();
        drawn.sort_by_key(|(seq, _)| *seq);
        let holds = |index: ModIndex, capability: Capability| arbiter.holds(index, capability);
        let mut layers = Vec::with_capacity(2 * drawn.len());
        layers.extend(
            drawn.iter().filter(|(_, index)| holds(*index, Capability::Hud)).map(|(_, index)| Self::hud(*index)),
        );
        layers.extend(
            drawn
                .iter()
                .filter(|(_, index)| holds(*index, Capability::Overlay))
                .map(|(_, index)| Self::overlay(*index)),
        );
        layers
    }

    /// True while an active Hud holder's claim asks for the vanilla HUD to be hidden.
    pub fn hides_vanilla_hud(registry: &ModRegistry, arbiter: &LeaseArbiter) -> bool {
        registry.iter().any(|registered| {
            arbiter.holds(registered.index, Capability::Hud)
                && registered
                    .descriptor
                    .claims
                    .iter()
                    .any(|claim| claim.capability == Capability::Hud && claim.hide_vanilla_hud)
        })
    }

    fn hud(mod_index: ModIndex) -> LayerEntry {
        LayerEntry { mod_index, kind: LayerKind::Hud, allowed: FrameInputData::LAYER_HUD }
    }

    fn overlay(mod_index: ModIndex) -> LayerEntry {
        LayerEntry { mod_index, kind: LayerKind::Overlay, allowed: FrameInputData::LAYER_OVERLAY }
    }
}
