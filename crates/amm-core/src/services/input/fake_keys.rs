use crate::structs::input::{FakeKeyOp, InputEvent};
use crate::structs::registry::{ModIndex, ModMask};

/// The keys mods hold down for the game with fake presses, and the presses still to send. One 256-bit set per
/// mod slot. A key goes down when the first mod holds it and up when the last mod lets it go. Game thread.
///
/// Bounds: `held` is one set per mod slot. A hold or a game up that finds `MAX_OPS` presses queued is refused. A
/// release always queues its up, and there are at most as many of those as keys held.
pub struct FakeKeys {
    held: [[u64; 4]; ModMask::CAPACITY],
    ops: Vec<FakeKeyOp>,
}

impl FakeKeys {
    pub const MAX_OPS: usize = 256;

    pub fn new() -> Self {
        Self { held: [[0; 4]; ModMask::CAPACITY], ops: Vec::with_capacity(Self::MAX_OPS) }
    }

    /// Holds `vk` for the mod. True when the mod did not hold it before. The down is queued when no mod held it.
    pub fn hold(&mut self, index: ModIndex, vk: u32) -> bool {
        let (Some((word, bit)), slot) = (Self::locate(vk), usize::from(index.0)) else {
            return false;
        };
        if slot >= self.held.len() || self.held[slot][word] & bit != 0 || self.ops.len() >= Self::MAX_OPS {
            return false;
        }
        let first = !self.is_held(vk);
        self.held[slot][word] |= bit;
        if first {
            self.ops.push(FakeKeyOp { vk, down: true });
        }
        true
    }

    /// Lets go of `vk` for the mod. True when the mod held it. The up is queued when no mod holds it now.
    pub fn release(&mut self, index: ModIndex, vk: u32) -> bool {
        let (Some((word, bit)), slot) = (Self::locate(vk), usize::from(index.0)) else {
            return false;
        };
        if slot >= self.held.len() || self.held[slot][word] & bit == 0 {
            return false;
        }
        self.held[slot][word] &= !bit;
        if !self.is_held(vk) {
            self.ops.push(FakeKeyOp { vk, down: false });
        }
        true
    }

    /// Lets go of every key of the mod. Returns the mod when it held any, else an empty set.
    pub fn release_mod(&mut self, index: ModIndex) -> ModMask {
        let mut lost = ModMask::default();
        for vk in 0..InputEvent::VK_SLOTS as u32 {
            if self.release(index, vk) {
                lost.insert(index);
            }
        }
        lost
    }

    /// Lets go of every key of every mod. Returns the mods that held any.
    pub fn release_all(&mut self) -> ModMask {
        let mut lost = ModMask::default();
        for slot in 0..ModMask::CAPACITY as u8 {
            lost.0 |= self.release_mod(ModIndex(slot)).0;
        }
        lost
    }

    /// Lets go of every key of every mod except `keep`. Returns the mods that held any. `keep` holds its keys, so a
    /// key it shares with a released mod stays down.
    pub fn release_all_except(&mut self, keep: ModIndex) -> ModMask {
        let mut lost = ModMask::default();
        for slot in 0..ModMask::CAPACITY as u8 {
            if slot != keep.0 {
                lost.0 |= self.release_mod(ModIndex(slot)).0;
            }
        }
        lost
    }

    /// True when any mod holds `vk`.
    pub fn is_held(&self, vk: u32) -> bool {
        Self::locate(vk).is_some_and(|(word, bit)| self.held.iter().any(|set| set[word] & bit != 0))
    }

    /// Moves the queued presses into `out`, oldest first.
    pub fn take_ops(&mut self, out: &mut Vec<FakeKeyOp>) {
        out.append(&mut self.ops);
    }

    /// How many presses wait. A mark for `drop_ups_since`.
    pub fn pending(&self) -> usize {
        self.ops.len()
    }

    /// Drops the ups queued after `mark` for keys `is_down` says are down for real: the real key-up lets the game
    /// see the key go up.
    pub fn drop_ups_since(&mut self, mark: usize, is_down: impl Fn(u32) -> bool) {
        let mut at = mark;
        while at < self.ops.len() {
            let op = self.ops[at];
            if !op.down && is_down(op.vk) {
                self.ops.remove(at);
            } else {
                at += 1;
            }
        }
    }

    /// Queues the up of a real key the game saw go down, so the game lets it go. Refused, with false, when
    /// `MAX_OPS` presses wait, as a hold is.
    pub fn queue_game_up(&mut self, vk: u32) -> bool {
        if Self::locate(vk).is_none() || self.ops.len() >= Self::MAX_OPS {
            return false;
        }
        self.ops.push(FakeKeyOp { vk, down: false });
        true
    }

    fn locate(vk: u32) -> Option<(usize, u64)> {
        (vk < InputEvent::VK_SLOTS as u32).then(|| ((vk / 64) as usize, 1u64 << (vk % 64)))
    }
}

impl Default for FakeKeys {
    fn default() -> Self {
        Self::new()
    }
}
