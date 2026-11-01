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
    /// layers in registration order, then Overlay layers of mods without an `Input` lease in
    /// registration order, then Overlay layers of `Input` holders with the most recent acquire on
    /// top. `registered_seq` gives the registration order of a mod.
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
        let input = arbiter.holders(Capability::Input);
        let holds = |index: ModIndex, capability: Capability| arbiter.holds(index, capability);
        let mut layers = Vec::with_capacity(2 * drawn.len());
        layers.extend(
            drawn.iter().filter(|(_, index)| holds(*index, Capability::Hud)).map(|(_, index)| Self::hud(*index)),
        );
        layers.extend(
            drawn
                .iter()
                .filter(|(_, index)| holds(*index, Capability::Overlay) && !input.contains(*index))
                .map(|(_, index)| Self::overlay(*index)),
        );
        let newest_first: Vec<ModIndex> = arbiter.most_recent(Capability::Input).collect();
        layers.extend(
            newest_first
                .iter()
                .rev()
                .filter(|index| holds(**index, Capability::Overlay) && drawn.iter().any(|(_, drawn)| drawn == *index))
                .map(|index| Self::overlay(*index)),
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

    /// True while an active (not suspended) Input holder's claim asks for the player's own turn to be blocked.
    pub fn blocks_player_turn(registry: &ModRegistry, arbiter: &LeaseArbiter) -> bool {
        registry.iter().any(|registered| {
            arbiter.holds(registered.index, Capability::Input)
                && registered
                    .descriptor
                    .claims
                    .iter()
                    .any(|claim| claim.capability == Capability::Input && claim.block_player_turn)
        })
    }

    fn hud(mod_index: ModIndex) -> LayerEntry {
        LayerEntry { mod_index, kind: LayerKind::Hud, allowed: FrameInputData::LAYER_HUD }
    }

    fn overlay(mod_index: ModIndex) -> LayerEntry {
        LayerEntry { mod_index, kind: LayerKind::Overlay, allowed: FrameInputData::LAYER_OVERLAY }
    }
}
