use crate::structs::registry::{ModIndex, ModMask};

/// The lease state of one capability. `acquired_order` lists the holders from the oldest
/// acquisition to the newest, packed at the front, and ends with `EMPTY` slots. It holds one slot
/// per possible mod, so it cannot grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityLeases {
    pub exclusive: Option<ModIndex>,
    pub shared: ModMask,
    pub suspended: ModMask,
    pub denied_since_grant: ModMask,
    pub acquired_order: [u8; ModMask::CAPACITY],
}

impl CapabilityLeases {
    pub const EMPTY: u8 = u8::MAX;

    pub fn new() -> Self {
        Self {
            exclusive: None,
            shared: ModMask::default(),
            suspended: ModMask::default(),
            denied_since_grant: ModMask::default(),
            acquired_order: [Self::EMPTY; ModMask::CAPACITY],
        }
    }

    /// The mods that hold a lease and are not suspended.
    pub fn active(&self) -> ModMask {
        let mut mask = self.shared;
        if let Some(holder) = self.exclusive {
            mask.insert(holder);
        }
        ModMask(mask.0 & !self.suspended.0)
    }

    /// Marks `who` as the newest holder.
    pub fn touch(&mut self, who: ModIndex) {
        self.forget(who);
        if let Some(slot) = self.acquired_order.iter_mut().find(|slot| **slot == Self::EMPTY) {
            *slot = who.0;
        }
    }

    /// Removes `who` from the acquisition order and keeps the rest packed.
    pub fn forget(&mut self, who: ModIndex) {
        let Some(position) = self.acquired_order.iter().position(|slot| *slot == who.0) else {
            return;
        };
        self.acquired_order.copy_within(position + 1.., position);
        if let Some(last) = self.acquired_order.last_mut() {
            *last = Self::EMPTY;
        }
    }
}

impl Default for CapabilityLeases {
    fn default() -> Self {
        Self::new()
    }
}
