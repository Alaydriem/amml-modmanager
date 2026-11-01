use std::collections::HashMap;

use amm_core::structs::mesh::TextureDelta;
use amm_core::structs::registry::ModMask;

use crate::types::AmmHostLayer;

/// The GPU texture memory of each owner: the 32 mod slots and the manager. Each owner may hold at most its byte
/// limit of textures in at most its count limit of textures: the defaults given to `new`, or the limits set for a
/// mod by `set_limit` (its grant) until `clear_limit`. A full upload that would go past either is refused and logged
/// once per owner, until the owner's textures are all freed. A partial upload only reaches a texture whose full
/// upload was admitted. Each owner's map is bound by its count limit.
pub struct TextureBudget {
    default_bytes: u64,
    default_textures: usize,
    bytes_per_owner: [u64; Self::OWNERS],
    textures_per_owner: [usize; Self::OWNERS],
    sizes: Vec<HashMap<u64, u64>>,
    used: [u64; Self::OWNERS],
    warned: u32,
}

impl TextureBudget {
    const OWNERS: usize = ModMask::CAPACITY + 1;
    const MANAGER_SLOT: usize = ModMask::CAPACITY;
    const BYTES_PER_PIXEL: u64 = 4;

    pub fn new(bytes_per_mod: u64, textures_per_mod: usize) -> Self {
        Self {
            default_bytes: bytes_per_mod,
            default_textures: textures_per_mod,
            bytes_per_owner: [bytes_per_mod; Self::OWNERS],
            textures_per_owner: [textures_per_mod; Self::OWNERS],
            sizes: (0..Self::OWNERS).map(|_| HashMap::new()).collect(),
            used: [0; Self::OWNERS],
            warned: 0,
        }
    }

    /// True when the delta may be uploaded. An admitted full upload counts against the owner's budget, and
    /// replaces the size of an earlier texture with the same id.
    pub fn admit(&mut self, owner: u8, delta: &TextureDelta) -> bool {
        let Some(slot) = Self::slot(owner) else {
            return false;
        };
        if !delta.full {
            return self.sizes[slot].contains_key(&delta.id);
        }
        let size = u64::from(delta.width) * u64::from(delta.height) * Self::BYTES_PER_PIXEL;
        if size == 0 {
            return false;
        }
        let previous = self.sizes[slot].get(&delta.id).copied();
        let next = self.used[slot] - previous.unwrap_or(0) + size;
        let count_full = previous.is_none() && self.sizes[slot].len() >= self.textures_per_owner[slot];
        if next > self.bytes_per_owner[slot] || count_full {
            self.refuse(slot, owner, size);
            return false;
        }
        self.sizes[slot].insert(delta.id, size);
        self.used[slot] = next;
        true
    }

    /// Gives the owner its own limits in place of the defaults. Textures it holds stay, also when they are now
    /// over the limits; later uploads are checked against the new limits.
    pub fn set_limit(&mut self, owner: u8, bytes: u64, textures: usize) {
        let Some(slot) = Self::slot(owner) else {
            return;
        };
        self.bytes_per_owner[slot] = bytes;
        self.textures_per_owner[slot] = textures;
    }

    /// Gives the owner the default limits again.
    pub fn clear_limit(&mut self, owner: u8) {
        self.set_limit(owner, self.default_bytes, self.default_textures);
    }

    pub fn free(&mut self, owner: u8, id: u64) {
        let Some(slot) = Self::slot(owner) else {
            return;
        };
        if let Some(size) = self.sizes[slot].remove(&id) {
            self.used[slot] -= size;
        }
    }

    /// Forgets every texture of the owner, which also lets a later refusal be logged again.
    pub fn free_owner(&mut self, owner: u8) {
        let Some(slot) = Self::slot(owner) else {
            return;
        };
        self.sizes[slot].clear();
        self.used[slot] = 0;
        self.warned &= !(1 << slot);
    }

    /// Forgets the textures of every owner, after the device dropped them all.
    pub fn free_all(&mut self) {
        for slot in 0..Self::OWNERS {
            self.sizes[slot].clear();
            self.used[slot] = 0;
        }
        self.warned = 0;
    }

    pub fn used(&self, owner: u8) -> u64 {
        Self::slot(owner).map_or(0, |slot| self.used[slot])
    }

    fn refuse(&mut self, slot: usize, owner: u8, size: u64) {
        if self.warned & (1 << slot) != 0 {
            return;
        }
        self.warned |= 1 << slot;
        curia::warn!("texture upload refused, it would go over the texture budget", {
            owner: owner,
            used: self.used[slot],
            upload: size,
            budget: self.bytes_per_owner[slot],
            textures: self.sizes[slot].len(),
            texture_budget: self.textures_per_owner[slot],
        });
    }

    fn slot(owner: u8) -> Option<usize> {
        match owner {
            AmmHostLayer::MANAGER => Some(Self::MANAGER_SLOT),
            owner if usize::from(owner) < ModMask::CAPACITY => Some(usize::from(owner)),
            _ => None,
        }
    }
}
