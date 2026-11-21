use crate::structs::event::{KeybindPhase, ModEvent};

/// A fixed ring of events for one mod. The bound is `capacity`. When the ring is full, `push` makes room by dropping
/// the oldest `Keybind` with phase `Pressed` or `Repeat`. A keybind release and a lease, enable, settings, menu entry
/// or language event is never dropped for it (see `ModEvent::is_protected`). If no such keybind is queued, an
/// unprotected new event is dropped, and a protected new event drops the oldest unprotected event. A queue full of
/// protected events drops the new event, which is the only way to keep the bound. A mod can see a keybind release
/// whose press was dropped when the queue was full, and must treat it as a no-op.
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

    fn drop_oldest_where(&mut self, matches: impl Fn(&ModEvent) -> bool) -> bool {
        let Some(logical) = self.oldest_where(matches) else {
            return false;
        };
        self.remove_at(logical);
        self.dropped += 1;
        true
    }

    fn make_room(&mut self, incoming: &ModEvent) -> bool {
        let unreleased =
            |e: &ModEvent| matches!(e, ModEvent::Keybind { phase, .. } if *phase != KeybindPhase::Released);
        if self.drop_oldest_where(unreleased) {
            return true;
        }
        incoming.is_protected() && self.drop_oldest_where(|e| !e.is_protected())
    }
}
