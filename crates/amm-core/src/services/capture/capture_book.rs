use std::path::Path;

use crate::errors::CaptureError;
use crate::services::event::EventHub;
use crate::services::lease::LeaseArbiter;
use crate::services::path::PathRules;
use crate::structs::capture::{CaptureEntry, CaptureRequest, CaptureStage, PendingCaptures, ScreenshotLayers};
use crate::structs::descriptor::Capability;
use crate::structs::event::ModEvent;
use crate::structs::registry::ModIndex;

/// The screenshot captures in flight, from the request to `ScreenshotWritten`. The table has `MAX_CAPTURES`
/// slots, one per overlay staging texture, so it cannot grow; a request past it, or a second request of a mod with a
/// capture in flight, is `Busy`. Game-only captures and captures with mod layers share the slots and the
/// one-per-mod rule. Sequences start at 1 and never take the value 0.
///
/// A capture is `Pending` until the overlay takes it. A pending capture whose mod loses its active `Hud` and
/// `Overlay` leases, or that the overlay has not taken within `PENDING_TICKS` manager ticks, is cancelled with
/// `ScreenshotTaken { ok: false }`; one whose mod unregisters is dropped with no event. A taken capture always
/// runs to its end, also when its mod loses the lease. When its mod unregisters it loses its owner and sends no
/// event: a capture still `Taken` is then not written, and one already `Writing` still writes its file.
pub struct CaptureBook {
    entries: [Option<CaptureEntry>; Self::MAX_CAPTURES],
    last_sequence: u32,
}

impl CaptureBook {
    pub const MAX_CAPTURES: usize = PendingCaptures::CAPACITY;
    /// The manager ticks a capture may stay `Pending`: 2 s at 20 client ticks a second. Past it the host is taken
    /// to draw no Presents, and the capture fails.
    pub const PENDING_TICKS: u32 = 40;

    pub fn new() -> Self {
        Self { entries: std::array::from_fn(|_| None), last_sequence: 0 }
    }

    /// Books a capture for `index` at the capture point `layers`, written to `path`, and returns its sequence.
    /// The mod must hold an active `Hud` or `Overlay` lease (`NotPermitted`) and the path must pass
    /// `PathRules::screenshot_fault` (`InvalidPath`). The caller checks that the mod is registered.
    pub fn request(
        &mut self,
        arbiter: &LeaseArbiter,
        index: ModIndex,
        path: &Path,
        layers: ScreenshotLayers,
    ) -> Result<u32, CaptureError> {
        if !Self::may_capture(arbiter, index) {
            return Err(CaptureError::NotPermitted);
        }
        if let Some(reason) = PathRules::screenshot_fault(path) {
            curia::warn!("screenshot path refused", { path: path.display().to_string(), reason: reason });
            return Err(CaptureError::InvalidPath);
        }
        let own = self.entries.iter().flatten().any(|entry| entry.owner == Some(index));
        let free = self.entries.iter_mut().find(|entry| entry.is_none());
        let (false, Some(free)) = (own, free) else {
            return Err(CaptureError::Busy);
        };
        self.last_sequence = self.last_sequence.wrapping_add(1).max(1);
        *free = Some(CaptureEntry::new(self.last_sequence, index, path.to_path_buf(), layers));
        Ok(self.last_sequence)
    }

    /// The captures that wait for the overlay, in table order. It reads the table and does not allocate.
    pub fn pending(&self) -> PendingCaptures {
        let mut out = PendingCaptures::new();
        for entry in self.entries.iter().flatten().filter(|entry| entry.stage == CaptureStage::Pending) {
            if let Some(owner) = entry.owner {
                out.push(CaptureRequest::new(entry.sequence, owner, entry.layers));
            }
        }
        out
    }

    /// The captures that wait for the overlay, now marked taken, so the next call does not return them again.
    pub fn take(&mut self) -> PendingCaptures {
        let out = self.pending();
        for entry in self.entries.iter_mut().flatten().filter(|entry| entry.stage == CaptureStage::Pending) {
            entry.stage = CaptureStage::Taken;
        }
        out
    }

    /// The file a capture is written to, while the capture is in flight and its mod is registered.
    pub fn path(&self, sequence: u32) -> Option<&Path> {
        self.find(sequence).filter(|entry| entry.owner.is_some()).map(|entry| entry.path.as_path())
    }

    /// The overlay copied a taken capture into the manager's buffer (`ok`) or failed to. The mod gets
    /// `ScreenshotTaken`. Returns true when the capture now waits for its write; a failed copy, or the copy of a
    /// mod that unregistered, ends the capture.
    pub fn taken(&mut self, sequence: u32, ok: bool, hub: &mut EventHub) -> bool {
        let Some(position) = self.position(sequence, CaptureStage::Taken) else {
            return false;
        };
        let owner = self.entries[position].as_ref().and_then(|entry| entry.owner);
        if let Some(owner) = owner {
            hub.push(owner, ModEvent::ScreenshotTaken { sequence, ok });
        }
        match (ok, owner, self.entries[position].as_mut()) {
            (true, Some(_), Some(entry)) => {
                entry.stage = CaptureStage::Writing;
                true
            }
            _ => {
                self.entries[position] = None;
                false
            }
        }
    }

    /// The I/O thread wrote the file of a capture (`ok`) or failed to. The capture ends and its mod gets
    /// `ScreenshotWritten`.
    pub fn written(&mut self, sequence: u32, ok: bool, hub: &mut EventHub) {
        let Some(position) = self.position(sequence, CaptureStage::Writing) else {
            return;
        };
        if let Some(owner) = self.entries[position].take().and_then(|entry| entry.owner) {
            hub.push(owner, ModEvent::ScreenshotWritten { sequence, ok });
        }
    }

    /// The mod in `index` unregisters: its pending capture is dropped, and a taken one loses its owner.
    pub fn forget(&mut self, index: ModIndex) {
        for slot in self.entries.iter_mut() {
            let Some(entry) = slot.as_mut().filter(|entry| entry.owner == Some(index)) else {
                continue;
            };
            if entry.stage == CaptureStage::Pending {
                *slot = None;
            } else {
                entry.owner = None;
            }
        }
    }

    /// Cancels each pending capture whose mod no longer holds an active `Hud` or `Overlay` lease, with
    /// `ScreenshotTaken { ok: false }`.
    pub fn cancel_unleased(&mut self, arbiter: &LeaseArbiter, hub: &mut EventHub) {
        for slot in self.entries.iter_mut() {
            let Some(entry) = slot.as_ref().filter(|entry| entry.stage == CaptureStage::Pending) else {
                continue;
            };
            let Some(owner) = entry.owner.filter(|owner| !Self::may_capture(arbiter, *owner)) else {
                continue;
            };
            hub.push(owner, ModEvent::ScreenshotTaken { sequence: entry.sequence, ok: false });
            *slot = None;
        }
    }

    /// One manager tick: each pending capture ages by one tick, and one at `PENDING_TICKS` is cancelled with
    /// `ScreenshotTaken { ok: false }`. It does not allocate.
    pub fn tick(&mut self, hub: &mut EventHub) {
        for slot in self.entries.iter_mut() {
            let Some(entry) = slot.as_mut().filter(|entry| entry.stage == CaptureStage::Pending) else {
                continue;
            };
            entry.pending_ticks += 1;
            if entry.pending_ticks < Self::PENDING_TICKS {
                continue;
            }
            curia::warn!("screenshot not taken by the host in time", {
                sequence: entry.sequence,
                ticks: entry.pending_ticks,
            });
            if let Some(owner) = entry.owner {
                hub.push(owner, ModEvent::ScreenshotTaken { sequence: entry.sequence, ok: false });
            }
            *slot = None;
        }
    }

    /// True when the mod in `index` holds an active `Hud` or `Overlay` lease.
    pub fn may_capture(arbiter: &LeaseArbiter, index: ModIndex) -> bool {
        arbiter.holds(index, Capability::Hud) || arbiter.holds(index, Capability::Overlay)
    }

    fn find(&self, sequence: u32) -> Option<&CaptureEntry> {
        self.entries.iter().flatten().find(|entry| entry.sequence == sequence)
    }

    fn position(&self, sequence: u32, stage: CaptureStage) -> Option<usize> {
        self.entries
            .iter()
            .position(|entry| entry.as_ref().is_some_and(|entry| entry.sequence == sequence && entry.stage == stage))
    }
}

impl Default for CaptureBook {
    fn default() -> Self {
        Self::new()
    }
}
