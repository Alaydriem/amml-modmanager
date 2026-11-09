use std::collections::VecDeque;

use crate::structs::input::{PointerButton, PointerOp, TurnAxis};
use crate::structs::registry::{ModIndex, ModMask};

/// The mouse buttons mods hold down for the game with fake presses, and the fake mouse actions still to send. One
/// button set per mod slot. A button goes down when the first mod holds it and up when the last mod lets it go; a
/// wheel turn is sent once and holds nothing. A look turn adds into one pending value per axis, which `take_ops`
/// hands over as at most one `Turn` per axis, after the queued actions; it never uses the queue. Game thread.
///
/// Bounds: `held` is one set per mod slot. `ops` holds at most `MAX_PENDING` actions. A new action into a queue at
/// `CAPACITY` evicts the oldest wheel turns until it fits; a button down or up is never evicted, so the game never
/// misses a button change `held` records. Without enough wheel turns to evict, the new action is refused and
/// nothing changes. `take_dropped` counts the evicted and the refused actions.
///
/// A forced release (`release_mod`, `release_all`, `release_all_except`) always clears the mod's buttons and queues
/// each up past `CAPACITY`, so no button stays down in the game. Only a button that goes from held to free gets an
/// up, and a button goes from free to held only through an action that leaves the queue at `CAPACITY` or below, so
/// at most one forced up per button waits past `CAPACITY`: `MAX_PENDING` is `CAPACITY` plus the three buttons.
pub struct FakePointer {
    held: [u8; ModMask::CAPACITY],
    ops: VecDeque<PointerOp>,
    // pending look turn in millidegrees, held within `MAX_TURN_MILLIDEGREES` either way
    pitch: i32,
    yaw: i32,
    dropped: u64,
}

impl FakePointer {
    pub const CAPACITY: usize = 256;
    /// The most wheel notches one `send_pointer` call may turn, either way.
    pub const MAX_WHEEL_STEPS: i32 = 32;
    /// The most millidegrees one `send_pointer` call may turn on one axis, either way.
    pub const MAX_TURN_MILLIDEGREES: i32 = 360_000;
    /// The most actions that wait: `CAPACITY`, plus one forced up for each button.
    pub const MAX_PENDING: usize = Self::CAPACITY + Self::BUTTONS.len();
    const BUTTONS: [PointerButton; 3] = [PointerButton::Left, PointerButton::Right, PointerButton::Middle];

    pub fn new() -> Self {
        Self {
            held: [0; ModMask::CAPACITY],
            ops: VecDeque::with_capacity(Self::CAPACITY),
            pitch: 0,
            yaw: 0,
            dropped: 0,
        }
    }

    /// Holds `button` for the mod. True when the mod did not hold it before. The down is queued when no mod held it;
    /// when the queue has no room for it, nothing changes and the answer is false.
    pub fn hold(&mut self, index: ModIndex, button: PointerButton) -> bool {
        let (slot, bit) = (usize::from(index.0), Self::bit(button));
        if slot >= self.held.len() || self.held[slot] & bit != 0 {
            return false;
        }
        if !self.is_held(button) && !self.push(PointerOp::Button { button, down: true }) {
            return false;
        }
        self.held[slot] |= bit;
        true
    }

    /// Lets go of `button` for the mod. True when the mod held it. The up is queued when no mod holds it now; when
    /// the queue has no room for it, the mod keeps the button and the answer is false.
    pub fn release(&mut self, index: ModIndex, button: PointerButton) -> bool {
        let (slot, bit) = (usize::from(index.0), Self::bit(button));
        if slot >= self.held.len() || self.held[slot] & bit == 0 {
            return false;
        }
        let shared = self.held.iter().enumerate().any(|(other, set)| other != slot && set & bit != 0);
        if !shared && !self.push(PointerOp::Button { button, down: false }) {
            return false;
        }
        self.held[slot] &= !bit;
        true
    }

    /// Queues a wheel turn of `steps` notches. False when the queue has no room for it.
    pub fn wheel(&mut self, steps: i32) -> bool {
        self.push(PointerOp::Wheel { steps })
    }

    /// Adds `millidegrees` to the pending look turn of `axis`. It uses no queue room, so it never fails; the sum
    /// is clamped to `MAX_TURN_MILLIDEGREES` either way, one full turn. The answer is always true.
    pub fn turn(&mut self, axis: TurnAxis, millidegrees: i32) -> bool {
        let pending = match axis {
            TurnAxis::Pitch => &mut self.pitch,
            TurnAxis::Yaw => &mut self.yaw,
        };
        *pending =
            pending.saturating_add(millidegrees).clamp(-Self::MAX_TURN_MILLIDEGREES, Self::MAX_TURN_MILLIDEGREES);
        true
    }

    /// True when a new action fits: the queue is below `CAPACITY`, or holds enough wheel turns to evict.
    pub fn has_room(&self) -> bool {
        let over = (self.ops.len() + 1).saturating_sub(Self::CAPACITY);
        over == 0 || self.ops.iter().filter(|op| matches!(op, PointerOp::Wheel { .. })).count() >= over
    }

    /// Queues the up of a real `button` the game saw go down, so the game lets it go. False when the queue has no
    /// room for it; nothing changes then.
    pub fn queue_game_up(&mut self, button: PointerButton) -> bool {
        self.push(PointerOp::Button { button, down: false })
    }

    /// Lets go of every button of the mod, past a full queue. Returns the mod when it held any, else an empty set.
    pub fn release_mod(&mut self, index: ModIndex) -> ModMask {
        let slot = usize::from(index.0);
        let Some(&set) = self.held.get(slot) else {
            return ModMask::default();
        };
        if set == 0 {
            return ModMask::default();
        }
        self.held[slot] = 0;
        for button in Self::BUTTONS {
            if set & Self::bit(button) != 0 && !self.is_held(button) {
                self.ops.push_back(PointerOp::Button { button, down: false });
            }
        }
        let mut lost = ModMask::default();
        lost.insert(index);
        lost
    }

    /// Lets go of every button of every mod. Returns the mods that held any.
    pub fn release_all(&mut self) -> ModMask {
        let mut lost = ModMask::default();
        for slot in 0..ModMask::CAPACITY as u8 {
            lost.0 |= self.release_mod(ModIndex(slot)).0;
        }
        lost
    }

    /// Lets go of every button of every mod except `keep`. Returns the mods that held any. A button `keep` shares
    /// with a released mod stays down.
    pub fn release_all_except(&mut self, keep: ModIndex) -> ModMask {
        let mut lost = ModMask::default();
        for slot in 0..ModMask::CAPACITY as u8 {
            if slot != keep.0 {
                lost.0 |= self.release_mod(ModIndex(slot)).0;
            }
        }
        lost
    }

    /// True when any mod holds `button`.
    pub fn is_held(&self, button: PointerButton) -> bool {
        let bit = Self::bit(button);
        self.held.iter().any(|set| set & bit != 0)
    }

    /// Moves the queued actions into `out`, oldest first, then the pending look turn: pitch, then yaw, each only
    /// when it is not 0. The pending turn is then 0.
    pub fn take_ops(&mut self, out: &mut Vec<PointerOp>) {
        out.extend(self.ops.drain(..));
        let pitch = std::mem::take(&mut self.pitch);
        let yaw = std::mem::take(&mut self.yaw);
        if pitch != 0 {
            out.push(PointerOp::Turn { axis: TurnAxis::Pitch, millidegrees: pitch });
        }
        if yaw != 0 {
            out.push(PointerOp::Turn { axis: TurnAxis::Yaw, millidegrees: yaw });
        }
    }

    /// How many actions wait: the queued ones, plus one for each axis with a pending turn.
    pub fn pending(&self) -> usize {
        self.ops.len() + usize::from(self.pitch != 0) + usize::from(self.yaw != 0)
    }

    /// How many actions a full queue evicted or refused since the last call.
    pub fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }

    // Queues `op` and leaves the queue at `CAPACITY` or below. A queue at `CAPACITY` evicts its oldest wheel turns
    // first; without enough of them, `op` is refused, nothing changes and false is returned.
    fn push(&mut self, op: PointerOp) -> bool {
        if !self.has_room() {
            self.dropped += 1;
            return false;
        }
        while self.ops.len() >= Self::CAPACITY {
            let Some(oldest_wheel) = self.ops.iter().position(|queued| matches!(queued, PointerOp::Wheel { .. }))
            else {
                break;
            };
            self.ops.remove(oldest_wheel);
            self.dropped += 1;
        }
        self.ops.push_back(op);
        true
    }

    fn bit(button: PointerButton) -> u8 {
        match button {
            PointerButton::Left => 1,
            PointerButton::Right => 2,
            PointerButton::Middle => 4,
        }
    }
}

impl Default for FakePointer {
    fn default() -> Self {
        Self::new()
    }
}
