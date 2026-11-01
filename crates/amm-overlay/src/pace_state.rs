use crate::pace_window::PaceWindow;

/// What `PresentPace` keeps between Presents, behind its lock: fixed counters only, so a Present allocates nothing.
pub(crate) struct PaceState {
    // the host forces the size of the bound chain now
    pub(crate) forced: bool,
    // sync interval in the high 32 bits and flags in the low 32 bits of the last Present; `None` for none
    pub(crate) last_key: Option<u64>,
    // the counting second
    pub(crate) window: PaceWindow,
    // the Presents of the forced span now running
    pub(crate) span: PaceWindow,
    // the distinct keys seen in the forced span, the first `SEEN` of them
    pub(crate) seen: [u64; Self::SEEN],
    pub(crate) seen_len: usize,
    // key changes in the counting second, and whether one of them was logged
    pub(crate) sync_changes: u32,
    pub(crate) change_logged: bool,
}

impl PaceState {
    pub(crate) const SEEN: usize = 8;

    pub(crate) const fn new() -> Self {
        Self {
            forced: false,
            last_key: None,
            window: PaceWindow::new(),
            span: PaceWindow::new(),
            seen: [0; Self::SEEN],
            seen_len: 0,
            sync_changes: 0,
            change_logged: false,
        }
    }

    /// Keeps `key` as seen in the forced span when it is new and there is room.
    pub(crate) fn see(&mut self, key: u64) {
        if self.seen[..self.seen_len].contains(&key) || self.seen_len == Self::SEEN {
            return;
        }
        self.seen[self.seen_len] = key;
        self.seen_len += 1;
    }
}
