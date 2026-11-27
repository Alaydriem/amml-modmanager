use std::collections::VecDeque;

use amm_abi::types::{AmmEvent, AmmEventKind, AmmHandle};
use amm_core::structs::event::{ModEvent, ReleaseReason};
use amml_api::amml_abi::{
    InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, ScreenshotTaken, ScreenshotWritten,
};
use amml_api::{Event, ShotId};

use super::ManagerTables;

/// The draining of manager events, and the reading of the loader events a link passes on. A loader event that an
/// earlier listener or the loader cancelled is never passed on.
pub struct EventDrain;

impl EventDrain {
    pub const CAPACITY: usize = 64;

    /// Drains every event queued for `handle` into `out`, in queue order. An event that does not convert is logged
    /// and skipped.
    pub fn poll(tables: &ManagerTables, handle: AmmHandle, out: &mut VecDeque<ModEvent>) {
        let Some(poll) = tables.core.poll_events else {
            return;
        };
        let blank =
            AmmEvent { kind: AmmEventKind::Enabled, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 };
        // On the stack, so a poll needs no shared buffer and no lock
        let mut buffer = [blank; Self::CAPACITY];
        loop {
            // SAFETY: the buffer has room for CAPACITY events and the manager keeps no pointer to it
            let count = unsafe { poll(handle, buffer.as_mut_ptr(), Self::CAPACITY) }.min(Self::CAPACITY);
            for event in &buffer[..count] {
                match ModEvent::try_from(event) {
                    Ok(event) => out.push_back(event),
                    Err(status) => curia::warn!("manager sent an event this client cannot read, event skipped", {
                        kind: event.kind as u8,
                        status: status as i32,
                    }),
                }
            }
            if count < Self::CAPACITY {
                return;
            }
        }
    }

    /// `FakeKeysReleased` for a release of the link's lease `lease`.
    pub fn released(event: &InjectedInputReleased, lease: Option<u64>) -> Option<ModEvent> {
        Self::of_lease(event, event.lease, lease)
            .then(|| ModEvent::FakeKeysReleased { reason: ReleaseReason::from(event.reason) })
    }

    /// `InputLeaseSuspended` for the link's lease `lease`.
    pub fn suspended(event: &InputLeaseSuspended, lease: Option<u64>) -> Option<ModEvent> {
        Self::of_lease(event, event.lease, lease).then_some(ModEvent::InputLeaseSuspended)
    }

    /// `InputLeaseResumed` for the link's lease `lease`.
    pub fn resumed(event: &InputLeaseResumed, lease: Option<u64>) -> Option<ModEvent> {
        Self::of_lease(event, event.lease, lease).then_some(ModEvent::InputLeaseResumed)
    }

    /// `InputLeaseLost` for the link's lease `lease`.
    pub fn lost(event: &InputLeaseLost, lease: Option<u64>) -> Option<ModEvent> {
        Self::of_lease(event, event.lease, lease).then_some(ModEvent::InputLeaseLost)
    }

    /// `ScreenshotTaken`; the loader raises it only to the mod that asked for the shot.
    pub fn taken(event: &ScreenshotTaken) -> Option<ModEvent> {
        (!event.is_cancelled()).then_some(ModEvent::ScreenshotTaken { id: ShotId(event.id), ok: event.ok != 0 })
    }

    /// `ScreenshotWritten`; the loader raises it only to the mod that asked for the shot.
    pub fn written(event: &ScreenshotWritten) -> Option<ModEvent> {
        (!event.is_cancelled()).then_some(ModEvent::ScreenshotWritten { id: ShotId(event.id), ok: event.ok != 0 })
    }

    fn of_lease<E: Event>(event: &E, event_lease: u64, lease: Option<u64>) -> bool {
        !event.is_cancelled() && lease == Some(event_lease)
    }
}
