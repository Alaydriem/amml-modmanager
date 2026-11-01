use crate::structs::registry::ModIndex;

/// A set of mod slots. One bit per slot, so it holds at most 32 mods.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModMask(pub u32);

impl ModMask {
    pub const CAPACITY: usize = u32::BITS as usize;

    pub fn insert(&mut self, index: ModIndex) {
        if let Some(bit) = Self::bit(index) {
            self.0 |= bit;
        }
    }

    pub fn remove(&mut self, index: ModIndex) {
        if let Some(bit) = Self::bit(index) {
            self.0 &= !bit;
        }
    }

    pub fn contains(&self, index: ModIndex) -> bool {
        Self::bit(index).is_some_and(|bit| self.0 & bit != 0)
    }

    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }

    pub fn len(&self) -> usize {
        self.0.count_ones() as usize
    }

    /// The members in ascending slot order.
    pub fn iter(&self) -> impl Iterator<Item = ModIndex> + use<> {
        let bits = self.0;
        (0..Self::CAPACITY as u8).filter(move |slot| bits & (1u32 << slot) != 0).map(ModIndex)
    }

    fn bit(index: ModIndex) -> Option<u32> {
        1u32.checked_shl(u32::from(index.0)).filter(|bit| *bit != 0)
    }
}
