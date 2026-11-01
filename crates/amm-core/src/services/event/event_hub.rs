use crate::services::event::EventQueue;
use crate::structs::event::ModEvent;
use crate::structs::registry::ModIndex;

/// One bounded `EventQueue` per mod slot, and a global sequence counter. The sequence starts at 1
/// and grows by one per push, so a lower number was pushed earlier. An index past `max_mods` has no
/// queue: a push to it is ignored and `queue_mut` returns an empty one-slot sink.
pub struct EventHub {
    queues: Vec<EventQueue>,
    last_sequence: Vec<u64>,
    sequence: u64,
    sink: EventQueue,
}

impl EventHub {
    pub fn new(max_mods: usize, capacity: usize) -> Self {
        Self {
            queues: (0..max_mods).map(|_| EventQueue::new(capacity)).collect(),
            last_sequence: vec![0; max_mods],
            sequence: 0,
            sink: EventQueue::new(1),
        }
    }

    pub fn push(&mut self, index: ModIndex, event: ModEvent) {
        let slot = usize::from(index.0);
        let Some(queue) = self.queues.get_mut(slot) else {
            return;
        };
        self.sequence += 1;
        self.last_sequence[slot] = self.sequence;
        queue.push(event);
    }

    pub fn queue_mut(&mut self, index: ModIndex) -> &mut EventQueue {
        match self.queues.get_mut(usize::from(index.0)) {
            Some(queue) => queue,
            None => {
                self.sink.clear();
                &mut self.sink
            }
        }
    }

    pub fn clear(&mut self, index: ModIndex) {
        if let Some(queue) = self.queues.get_mut(usize::from(index.0)) {
            queue.clear();
        }
    }

    /// The events lost to full queues since the hub was made, over all mods.
    pub fn dropped(&self) -> u64 {
        self.queues.iter().map(EventQueue::dropped).sum()
    }

    /// The sequence number of the newest event pushed to this mod, or 0 when none was.
    pub fn last_sequence(&self, index: ModIndex) -> u64 {
        self.last_sequence.get(usize::from(index.0)).copied().unwrap_or(0)
    }
}
