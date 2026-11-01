use std::collections::BTreeMap;

use serde::Deserialize;

use crate::config::{ExclusiveResolution, PairKey, SuspensionResolution};

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
