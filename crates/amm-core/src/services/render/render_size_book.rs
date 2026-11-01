use crate::errors::ManagerError;
use crate::services::lease::LeaseArbiter;
use crate::structs::descriptor::Capability;
use crate::structs::registry::ModIndex;
use crate::structs::render::{RenderSize, RenderSizeState};

/// The back-buffer size a `RenderSize` holder forces, and the size messages the game needs for it. The host
/// applies it: every `ResizeBuffers` of the game chain gets the forced size (`size_for`), and the size message a
/// frame returns is posted to the game window as `WM_SIZE`, so the game sets its viewport for that size.
///
/// A forced size ends with `set(.., None)` or when its holder no longer holds `RenderSize` (a release, an
/// unregister, a disable): the next frame then gives the game the window's own client size back. A 0 x 0 client
/// size (a minimised window) is never sent: the message waits for a frame with a real size. While a size is
/// forced, the forced size is sent again when the client size changes (the window was resized), on the next frame
/// after `window_changed` (a DPI, display or window position change), and at least every `KEEP_ALIVE_MS`, so the
/// game does not keep a size of its own as its viewport. State is fixed in size.
///
/// `shutdown` closes the book for good. A closed book forces nothing: `set` with a size is
/// `RenderSizeClosed`, `set` with `None` is a no-op, `frame` only records the client size and `busy` is false.
pub struct RenderSizeBook {
    forced: Option<(ModIndex, RenderSize)>,
    // the window client size of the last frame, 0 x 0 before the first frame and while the window is minimised
    client: RenderSize,
    // the forced size waits to be sent to the game
    force_due: bool,
    // the frame time in milliseconds the forced size was last sent at
    sent_ms: u64,
    // the client size waits to be sent to the game, after a forced size ended
    restore_due: bool,
    // the manager stopped; nothing is forced or owed from now on
    closed: bool,
}

impl RenderSizeBook {
    /// The longest time a forced size goes without being sent to the game again.
    pub const KEEP_ALIVE_MS: u64 = 1000;

    pub fn new() -> Self {
        Self {
            forced: None,
            client: RenderSize::default(),
            force_due: false,
            sent_ms: 0,
            restore_due: false,
            closed: false,
        }
    }

    /// Forces `size` for `who`, or ends the size `who` forces with `None`. `None` from a mod that forces nothing
    /// changes nothing. A size with a side of 0 or above `RenderSize::MAX_SIDE` is `RenderSizeOutOfRange`. The
    /// caller checks that `who` holds `RenderSize`. A closed book refuses a size with `RenderSizeClosed` and takes
    /// `None` as a no-op.
    pub fn set(&mut self, who: ModIndex, size: Option<RenderSize>) -> Result<(), ManagerError> {
        if self.closed {
            return if size.is_some() { Err(ManagerError::RenderSizeClosed) } else { Ok(()) };
        }
        match size {
            Some(size) if !size.is_forcible() => Err(ManagerError::RenderSizeOutOfRange),
            Some(size) => {
                self.forced = Some((who, size));
                self.force_due = true;
                self.restore_due = false;
                Ok(())
            }
            None => {
                if self.owner() == Some(who) {
                    self.end();
                }
                Ok(())
            }
        }
    }

    /// Ends the forced size when its holder no longer holds `RenderSize`.
    pub fn drop_unleased(&mut self, arbiter: &LeaseArbiter) {
        if let Some(owner) = self.owner()
            && !arbiter.holds(owner, Capability::RenderSize)
        {
            self.end();
        }
    }

    /// One frame at `now_ms` with the window client size `client`. Returns the size message to send the game now:
    /// the forced size after `set`, and again while forced after the client size changed, after `window_changed`,
    /// or when `KEEP_ALIVE_MS` passed since it was last sent; the client size after a forced size ended. Nothing
    /// while `client` is 0 x 0; a due message waits for the next real size. A closed book returns nothing.
    pub fn frame(&mut self, client: RenderSize, now_ms: u64) -> Option<RenderSize> {
        let resized = client != self.client;
        self.client = client;
        if self.closed || client.is_empty() {
            return None;
        }
        if let Some((_, size)) = self.forced {
            let stale = now_ms.saturating_sub(self.sent_ms) >= Self::KEEP_ALIVE_MS;
            let send = self.force_due || resized || stale;
            self.force_due = false;
            if send {
                self.sent_ms = now_ms;
            }
            return send.then_some(size);
        }
        if self.restore_due {
            self.restore_due = false;
            return Some(client);
        }
        None
    }

    /// The game window had a DPI, display or position change, which can make the game set its viewport from the
    /// window without a `WM_SIZE`. While a size is forced, the next frame sends it again. Otherwise nothing changes.
    pub fn window_changed(&mut self) {
        if !self.closed && self.forced.is_some() {
            self.force_due = true;
        }
    }

    /// The forced size, while one is in effect.
    pub fn forced(&self) -> Option<RenderSize> {
        self.forced.map(|(_, size)| size)
    }

    /// True while a size is forced or a size message waits, so every frame must reach the book.
    pub fn busy(&self) -> bool {
        self.forced.is_some() || self.restore_due
    }

    /// What `who` reads back, with the back buffer `actual` of the last Present in DXGI `format`.
    pub fn state(&self, who: ModIndex, actual: RenderSize, format: u32) -> RenderSizeState {
        let wanted = self.forced.filter(|(owner, _)| *owner == who).map(|(_, size)| size);
        RenderSizeState { wanted, actual, format }
    }

    /// The size a `ResizeBuffers` that asks for `asked` must use: the forced size while one is in effect.
    pub fn size_for(forced: Option<RenderSize>, asked: RenderSize) -> RenderSize {
        forced.unwrap_or(asked)
    }

    /// The size a `WM_SIZE` of `asked` gives the game, and whether a size is forced: the forced size while one is
    /// in effect, so a resize of the window keeps it. A minimised window passes as it is, so the game sees it.
    pub fn window_size_for(forced: Option<RenderSize>, minimised: bool, asked: RenderSize) -> (bool, RenderSize) {
        match forced {
            Some(size) if !minimised => (true, size),
            _ => (false, asked),
        }
    }

    /// Where the game's mouse feed puts the real cursor at back-buffer `point`, or `None` to leave the game's own
    /// point. `mapped` is the point of the full window a game viewport gives for it. With no size forced the game
    /// sees its window at the `client` size, so a mapped point is scaled from `back_buffer` to `client` and an
    /// unmapped one is left alone. With a size forced the game sees its window at the back-buffer size: every
    /// point is given in back-buffer pixels, `mapped` or else `point` itself. `None` for an empty size.
    pub fn game_point(
        &self,
        point: (f32, f32),
        mapped: Option<(f32, f32)>,
        back_buffer: RenderSize,
        client: RenderSize,
    ) -> Option<(f32, f32)> {
        if back_buffer.is_empty() || client.is_empty() {
            return None;
        }
        if self.forced.is_some() {
            return Some(mapped.unwrap_or(point));
        }
        let (x, y) = mapped?;
        let to_client_x = client.width as f32 / back_buffer.width as f32;
        let to_client_y = client.height as f32 / back_buffer.height as f32;
        Some((x * to_client_x, y * to_client_y))
    }

    /// The manager stops: the book closes for good, forces nothing and owes nothing. Returns the size message the
    /// game needs to get its own size back, the `client` size, only when a size was forced or a restore waited and
    /// `client` is not 0 x 0 (a minimised window takes its size from its own `WM_SIZE` on restore). Afterwards
    /// `set` with a size is `RenderSizeClosed`.
    pub fn shutdown(&mut self, client: RenderSize) -> Option<RenderSize> {
        let due = self.busy() && !client.is_empty();
        self.closed = true;
        self.forced = None;
        self.force_due = false;
        self.restore_due = false;
        due.then_some(client)
    }

    /// Opens a closed book again, for a host that stayed alive through a manager disable. `shutdown` left the book
    /// empty, so only the latch changes.
    pub fn open(&mut self) {
        self.closed = false;
    }

    fn owner(&self) -> Option<ModIndex> {
        self.forced.map(|(owner, _)| owner)
    }

    fn end(&mut self) {
        self.forced = None;
        self.force_due = false;
        self.restore_due = true;
    }
}

impl Default for RenderSizeBook {
    fn default() -> Self {
        Self::new()
    }
}
