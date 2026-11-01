use amm_core::structs::frame::{UiEvent, UiEventKind};

/// The routed input events of one receiver for its next egui pass. It holds at most `capacity` events: a pointer
/// move replaces a pointer move at the end, and a push into a full buffer drops the oldest event and counts it.
/// The storage is reserved at construction, so a push never allocates.
pub struct UiEventBuffer {
    events: Vec<UiEvent>,
    capacity: usize,
    dropped: u64,
}

impl UiEventBuffer {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.max(1);
        Self { events: Vec::with_capacity(capacity), capacity, dropped: 0 }
    }

    pub fn push(&mut self, event: UiEvent) {
        if event.kind == UiEventKind::PointerMove
            && let Some(last) = self.events.last_mut().filter(|last| last.kind == UiEventKind::PointerMove)
        {
            *last = event;
            return;
        }
        if self.events.len() == self.capacity {
            self.events.remove(0);
            self.dropped += 1;
        }
        self.events.push(event);
    }

    pub fn as_slice(&self) -> &[UiEvent] {
        &self.events
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    pub fn dropped(&self) -> u64 {
        self.dropped
    }
}
