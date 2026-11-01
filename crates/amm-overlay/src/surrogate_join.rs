use std::sync::atomic::{AtomicU32, Ordering};

/// Joins the two UTF-16 units of a character outside the basic plane, as `WM_CHAR` gives them. Used on the window
/// thread only.
pub(crate) struct SurrogateJoin {
    // the high half waiting for its low half; 0 when none
    high: AtomicU32,
}

impl SurrogateJoin {
    const HIGH_FIRST: u32 = 0xD800;
    const HIGH_LAST: u32 = 0xDBFF;
    const LOW_FIRST: u32 = 0xDC00;
    const LOW_LAST: u32 = 0xDFFF;
    const PLANE_BASE: u32 = 0x10000;
    const SHIFT: u32 = 10;

    pub(crate) const fn new() -> Self {
        Self { high: AtomicU32::new(0) }
    }

    /// The code point `unit` completes, or `None` for a high half, which waits, and for a low half with no high
    /// half before it, which is dropped. A waiting high half followed by a plain unit is dropped.
    pub(crate) fn feed(&self, unit: u32) -> Option<u32> {
        if (Self::HIGH_FIRST..=Self::HIGH_LAST).contains(&unit) {
            self.high.store(unit, Ordering::Relaxed);
            return None;
        }
        let high = self.high.swap(0, Ordering::Relaxed);
        if (Self::LOW_FIRST..=Self::LOW_LAST).contains(&unit) {
            if high == 0 {
                return None;
            }
            return Some(Self::PLANE_BASE + ((high - Self::HIGH_FIRST) << Self::SHIFT) + (unit - Self::LOW_FIRST));
        }
        Some(unit)
    }

    /// Forgets a waiting high half, on focus loss.
    pub(crate) fn reset(&self) {
        self.high.store(0, Ordering::Relaxed);
    }
}
