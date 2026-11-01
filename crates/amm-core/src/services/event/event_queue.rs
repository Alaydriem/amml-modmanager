use crate::structs::event::{KeybindPhase, ModEvent};

/// A fixed ring of events for one mod. The bound is `capacity`. When the ring is full, `push` makes
/// room in this order: (1) merge the two oldest adjacent `PointerDelta` events (no event is lost, so it
/// is not counted as a drop); (2) drop the oldest `Keybind` with phase `Pressed` or `Repeat`; (3) drop the
/// oldest `Wheel`, then the oldest `Key` with `down: true`. A key-up, a button-up, a keybind release and a
/// lease, enable or settings event is never dropped for a lower-ranked event (see `ModEvent::is_protected`).
/// If none of those exist, an unprotected new event is dropped. A protected new event then drops the oldest
/// button-down `PointerButton`, then the oldest unprotected event. A queue full of protected events drops
/// the new event, which is the only way to keep the bound. A mod can see a key-up whose key-down was dropped
/// when the queue was full, and must treat it as a no-op.
pub struct EventQueue {
    slots: Vec<ModEvent>,
    head: usize,
    len: usize,
    dropped: u64,
}

impl EventQueue {
    pub fn new(capacity: usize) -> Self {
        Self { slots: vec![ModEvent::Enabled; capacity.max(1)], head: 0, len: 0, dropped: 0 }
    }

    pub fn push(&mut self, event: ModEvent) {
        if self.len == self.slots.len() && !self.make_room(&event) {
            self.dropped += 1;
            return;
        }
        let slot = self.slot(self.len);
        self.slots[slot] = event;
        self.len += 1;
    }

    pub fn pop(&mut self) -> Option<ModEvent> {
        if self.len == 0 {
            return None;
        }
        let event = self.slots[self.head];
        self.head = (self.head + 1) % self.slots.len();
        self.len -= 1;
        Some(event)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// The number of events lost to a full queue since the queue was made.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    pub fn clear(&mut self) {
        self.head = 0;
        self.len = 0;
    }

    fn slot(&self, logical: usize) -> usize {
        (self.head + logical) % self.slots.len()
    }

    fn remove_at(&mut self, logical: usize) {
        for i in logical..self.len - 1 {
            let (to, from) = (self.slot(i), self.slot(i + 1));
            self.slots[to] = self.slots[from];
        }
        self.len -= 1;
    }

    fn oldest_where(&self, matches: impl Fn(&ModEvent) -> bool) -> Option<usize> {
        (0..self.len).find(|logical| matches(&self.slots[self.slot(*logical)]))
    }

    fn merge_oldest_deltas(&mut self) -> bool {
        let adjacent = (0..self.len.saturating_sub(1)).find_map(|i| {
            match (self.slots[self.slot(i)], self.slots[self.slot(i + 1)]) {
                (ModEvent::PointerDelta { dx: ax, dy: ay }, ModEvent::PointerDelta { dx: bx, dy: by }) => {
                    Some((i, ax + bx, ay + by))
                }
                _ => None,
            }
        });
        let Some((i, dx, dy)) = adjacent else {
            return false;
        };
        let target = self.slot(i);
        self.slots[target] = ModEvent::PointerDelta { dx, dy };
        self.remove_at(i + 1);
        true
    }

    fn drop_oldest_where(&mut self, matches: impl Fn(&ModEvent) -> bool) -> bool {
        let Some(logical) = self.oldest_where(matches) else {
            return false;
        };
        self.remove_at(logical);
        self.dropped += 1;
        true
    }

    fn make_room(&mut self, incoming: &ModEvent) -> bool {
        if self.merge_oldest_deltas() {
            return true;
        }
        let unreleased =
            |e: &ModEvent| matches!(e, ModEvent::Keybind { phase, .. } if *phase != KeybindPhase::Released);
        if self.drop_oldest_where(unreleased)
            || self.drop_oldest_where(|e| matches!(e, ModEvent::Wheel { .. }))
            || self.drop_oldest_where(|e| matches!(e, ModEvent::Key { down: true, .. }))
        {
            return true;
        }
        if !incoming.is_protected() {
            return false;
        }
        self.drop_oldest_where(|e| matches!(e, ModEvent::PointerButton { down: true, .. }))
            || self.drop_oldest_where(|e| !e.is_protected())
    }
}
