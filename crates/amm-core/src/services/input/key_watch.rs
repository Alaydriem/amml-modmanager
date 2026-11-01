use crate::structs::input::InputEvent;
use crate::structs::registry::{ModIndex, ModMask};

/// The keys each mod asked to see, whatever else the key does. One 256-bit set per mod slot. Game thread.
///
/// Bounds: one set per mod slot, at most `MAX_KEYS` keys in each.
pub struct KeyWatch {
    watched: [[u64; 4]; ModMask::CAPACITY],
}

impl KeyWatch {
    pub const MAX_KEYS: usize = 32;

    pub fn new() -> Self {
        Self { watched: [[0; 4]; ModMask::CAPACITY] }
    }

    /// Replaces the set of the mod. Codes of 256 and more, and keys past `MAX_KEYS`, are left out.
    pub fn set(&mut self, index: ModIndex, vks: &[u32]) {
        let Some(set) = self.watched.get_mut(usize::from(index.0)) else {
            return;
        };
        *set = [0; 4];
        for vk in vks.iter().take(Self::MAX_KEYS).filter(|vk| **vk < InputEvent::VK_SLOTS as u32) {
            set[(*vk / 64) as usize] |= 1u64 << (*vk % 64);
        }
    }

    pub fn forget(&mut self, index: ModIndex) {
        self.set(index, &[]);
    }

    /// The mods that watch `vk`.
    pub fn watchers(&self, vk: u32) -> ModMask {
        let mut mask = ModMask::default();
        if vk >= InputEvent::VK_SLOTS as u32 {
            return mask;
        }
        let (word, bit) = ((vk / 64) as usize, 1u64 << (vk % 64));
        for (slot, set) in self.watched.iter().enumerate() {
            if set[word] & bit != 0 {
                mask.insert(ModIndex(slot as u8));
            }
        }
        mask
    }
}

impl Default for KeyWatch {
    fn default() -> Self {
        Self::new()
    }
}
