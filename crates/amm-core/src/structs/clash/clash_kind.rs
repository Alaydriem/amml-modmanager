use crate::structs::clash::BindRef;
use crate::structs::descriptor::{Capability, Chord};
use crate::structs::registry::ModIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClashKind {
    Exclusive { capability: Capability, a: ModIndex, b: ModIndex },
    Suspension { capability: Capability, exclusive: ModIndex, shared: ModIndex },
    Key { chord: Chord, a: BindRef, b: BindRef },
}

impl ClashKind {
    /// True when the mod takes part in this clash.
    pub fn involves(&self, index: ModIndex) -> bool {
        match *self {
            Self::Exclusive { a, b, .. } => a == index || b == index,
            Self::Suspension { exclusive, shared, .. } => exclusive == index || shared == index,
            Self::Key { a, b, .. } => a.mod_index == index || b.mod_index == index,
        }
    }
}
