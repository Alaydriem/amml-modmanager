use std::fmt::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::structs::registry::{ModIndex, ModMask};

/// Counters for the hot paths. Every counter is a relaxed atomic, so the game thread and the render
/// thread record without a lock. One slot per possible mod, so nothing grows. `report` builds text
/// only when it is called.
pub struct Telemetry {
    frame_ns: [AtomicU64; ModMask::CAPACITY],
    vertices: [AtomicU64; ModMask::CAPACITY],
    texture_bytes: [AtomicU64; ModMask::CAPACITY],
    retained_bytes: [AtomicU64; ModMask::CAPACITY],
    event_drops: AtomicU64,
    vertex_skips: AtomicU64,
    texture_refusals: AtomicU64,
    freed_drops: AtomicU64,
    route_ns_total: AtomicU64,
    routes: AtomicU64,
}

impl Telemetry {
    pub fn new() -> Self {
        Self {
            frame_ns: std::array::from_fn(|_| AtomicU64::new(0)),
            vertices: std::array::from_fn(|_| AtomicU64::new(0)),
            texture_bytes: std::array::from_fn(|_| AtomicU64::new(0)),
            retained_bytes: std::array::from_fn(|_| AtomicU64::new(0)),
            event_drops: AtomicU64::new(0),
            vertex_skips: AtomicU64::new(0),
            texture_refusals: AtomicU64::new(0),
            freed_drops: AtomicU64::new(0),
            route_ns_total: AtomicU64::new(0),
            routes: AtomicU64::new(0),
        }
    }

    pub fn new_shared() -> Arc<Self> {
        Arc::new(Self::new())
    }

    /// The last frame callback of a mod: its time and its vertex count.
    pub fn record_frame(&self, index: ModIndex, frame_ns: u64, vertices: u64) {
        if let Some(slot) = Self::slot(index) {
            self.frame_ns[slot].store(frame_ns, Ordering::Relaxed);
            self.vertices[slot].store(vertices, Ordering::Relaxed);
        }
    }

    /// The GPU texture memory a mod uses now.
    pub fn record_texture_bytes(&self, index: ModIndex, bytes: u64) {
        if let Some(slot) = Self::slot(index) {
            self.texture_bytes[slot].store(bytes, Ordering::Relaxed);
        }
    }

    /// The events lost to full queues since the start, over all mods.
    pub fn record_event_drops(&self, total: u64) {
        self.event_drops.store(total, Ordering::Relaxed);
    }

    /// The host memory the compositor keeps for a mod's frame copies now.
    pub fn record_retained_bytes(&self, index: ModIndex, bytes: u64) {
        if let Some(slot) = Self::slot(index) {
            self.retained_bytes[slot].store(bytes, Ordering::Relaxed);
        }
    }

    pub fn retained_bytes(&self, index: ModIndex) -> u64 {
        Self::slot(index).map_or(0, |slot| self.retained_bytes[slot].load(Ordering::Relaxed))
    }

    /// Freed texture ids dropped because a frame freed more than the per-frame cap.
    pub fn record_freed_drops(&self, count: u64) {
        self.freed_drops.fetch_add(count, Ordering::Relaxed);
    }

    pub fn freed_drops(&self) -> u64 {
        self.freed_drops.load(Ordering::Relaxed)
    }

    /// A mod frame drawn without its mesh because it was over the vertex, index or command limit.
    pub fn record_vertex_skip(&self) {
        self.vertex_skips.fetch_add(1, Ordering::Relaxed);
    }

    /// A texture upload refused because it would go over the mod's texture budget.
    pub fn record_texture_refusal(&self) {
        self.texture_refusals.fetch_add(1, Ordering::Relaxed);
    }

    pub fn vertex_skips(&self) -> u64 {
        self.vertex_skips.load(Ordering::Relaxed)
    }

    pub fn texture_refusals(&self) -> u64 {
        self.texture_refusals.load(Ordering::Relaxed)
    }

    pub fn record_route(&self, route_ns: u64) {
        self.route_ns_total.fetch_add(route_ns, Ordering::Relaxed);
        self.routes.fetch_add(1, Ordering::Relaxed);
    }

    /// Sets the counters of a slot back to zero, for a mod that left.
    pub fn clear_mod(&self, index: ModIndex) {
        self.record_frame(index, 0, 0);
        self.record_texture_bytes(index, 0);
    }

    pub fn event_drops(&self) -> u64 {
        self.event_drops.load(Ordering::Relaxed)
    }

    /// The mean time of one routed event, or 0 before the first.
    pub fn route_mean_ns(&self) -> u64 {
        let routes = self.routes.load(Ordering::Relaxed);
        self.route_ns_total.load(Ordering::Relaxed).checked_div(routes).unwrap_or(0)
    }

    /// One line of text with every counter. Slots with no recorded values are left out.
    pub fn report(&self) -> String {
        let mut text = String::new();
        let _ = write!(
            text,
            "event_drops={} route_mean_ns={} vertex_skips={} texture_refusals={} freed_drops={}",
            self.event_drops(),
            self.route_mean_ns(),
            self.vertex_skips(),
            self.texture_refusals(),
            self.freed_drops(),
        );
        for slot in 0..ModMask::CAPACITY {
            let frame_ns = self.frame_ns[slot].load(Ordering::Relaxed);
            let vertices = self.vertices[slot].load(Ordering::Relaxed);
            let texture_bytes = self.texture_bytes[slot].load(Ordering::Relaxed);
            let retained = self.retained_bytes[slot].load(Ordering::Relaxed);
            if frame_ns == 0 && vertices == 0 && texture_bytes == 0 && retained == 0 {
                continue;
            }
            let _ = write!(
                text,
                " mod{slot}=[frame_ns={frame_ns} vertices={vertices} texture_bytes={texture_bytes} retained={retained}]"
            );
        }
        text
    }

    fn slot(index: ModIndex) -> Option<usize> {
        Some(usize::from(index.0)).filter(|slot| *slot < ModMask::CAPACITY)
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}
