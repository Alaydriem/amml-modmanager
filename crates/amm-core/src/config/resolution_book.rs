use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::{ExclusiveResolution, PairKey, SuspensionResolution};
use crate::structs::descriptor::Capability;

/// The operator's stored resolutions. Entries for pairs that are no longer registered stay in place.
/// The book grows only by operator action, one entry per reviewed pair.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ResolutionBook {
    exclusive: BTreeMap<PairKey, ExclusiveResolution>,
    suspension: BTreeMap<PairKey, SuspensionResolution>,
}

impl ResolutionBook {
    pub fn new() -> Self {
        Self::default()
    }

    /// True when no resolution of either kind is stored.
    pub fn is_empty(&self) -> bool {
        self.exclusive.is_empty() && self.suspension.is_empty()
    }

    pub fn exclusive_for(&self, key: &PairKey) -> Option<&ExclusiveResolution> {
        self.exclusive.get(key)
    }

    pub fn suspension_for(&self, key: &PairKey) -> Option<&SuspensionResolution> {
        self.suspension.get(key)
    }

    /// Each stored exclusive resolution of `capability` that prefers one mod of its pair, as `(preferred, other)`.
    pub fn preferences(&self, capability: Capability) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.exclusive.iter().filter(move |(key, _)| key.capability() == capability).filter_map(|(key, resolution)| {
            let ExclusiveResolution::Prefer(id) = resolution else {
                return None;
            };
            let (first, second) = key.ids();
            if id == first {
                Some((first, second))
            } else if id == second {
                Some((second, first))
            } else {
                None
            }
        })
    }

    /// Each pair of `capability` whose stored suspension resolution is `Coexist`, as its two sorted ids.
    pub fn coexist_pairs(&self, capability: Capability) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.suspension
            .iter()
            .filter(move |(key, resolution)| {
                key.capability() == capability && **resolution == SuspensionResolution::Coexist
            })
            .map(|(key, _)| key.ids())
    }

    pub fn with_exclusive(&self, key: PairKey, resolution: ExclusiveResolution) -> ResolutionBook {
        let mut next = self.clone();
        next.exclusive.insert(key, resolution);
        next
    }

    pub fn with_suspension(&self, key: PairKey, resolution: SuspensionResolution) -> ResolutionBook {
        let mut next = self.clone();
        next.suspension.insert(key, resolution);
        next
    }
}
