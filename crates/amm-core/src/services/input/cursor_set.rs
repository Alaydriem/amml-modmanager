use crate::structs::registry::{ModIndex, ModMask};

/// The mods that ask for a free cursor. One bit per mod slot, so it is bounded by the mask width.
pub struct CursorSet {
    requests: ModMask,
}

impl CursorSet {
    pub fn new() -> Self {
        Self { requests: ModMask::default() }
    }

    pub fn request(&mut self, index: ModIndex, on: bool) {
        if on {
            self.requests.insert(index);
        } else {
            self.requests.remove(index);
        }
    }

    /// Takes the mod out of the set, for a released or lost lease.
    pub fn remove(&mut self, index: ModIndex) {
        self.requests.remove(index);
    }

    /// True while any mod asks for the cursor or the manager panel is open.
    pub fn cursor_free(&self, panel_open: bool) -> bool {
        panel_open || !self.requests.is_empty()
    }
}

impl Default for CursorSet {
    fn default() -> Self {
        Self::new()
    }
}
