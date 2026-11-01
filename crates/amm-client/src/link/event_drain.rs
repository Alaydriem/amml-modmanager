use amm_abi::types::{AmmEvent, AmmHandle};
use amm_core::structs::event::ModEvent;

use super::ApiTable;

/// The reused buffer events are drained into. One poll takes at most `CAPACITY` events; the rest stay
/// queued in the manager for the next poll.
pub struct EventDrain {
    buffer: Vec<AmmEvent>,
}

impl EventDrain {
    pub const CAPACITY: usize = 64;

    pub fn new() -> Self {
        Self { buffer: vec![AmmEvent::from(ModEvent::Enabled); Self::CAPACITY] }
    }

    /// Drains up to `CAPACITY` events for `handle` and yields them as core events, in queue order. An event
    /// that does not convert is logged and skipped.
    pub fn poll(&mut self, table: &ApiTable, handle: AmmHandle) -> impl Iterator<Item = ModEvent> + '_ {
        let count = match table.api().poll_events {
            // SAFETY: the buffer has room for CAPACITY events and the manager keeps no pointer to it
            Some(poll) => unsafe { poll(handle, self.buffer.as_mut_ptr(), Self::CAPACITY) },
            None => 0,
        };
        self.buffer[..count.min(Self::CAPACITY)].iter().filter_map(|event| match ModEvent::try_from(event) {
            Ok(event) => Some(event),
            Err(status) => {
                curia::warn!("manager sent an event this client cannot read, event skipped", {
                    kind: event.kind as u8,
                    status: status as i32,
                });
                None
            }
        })
    }
}

impl Default for EventDrain {
    fn default() -> Self {
        Self::new()
    }
}
