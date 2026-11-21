use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use amm_ffi::services::HostCell;
use amml_api::{Client, EventCtx, Screen};

use super::{ClientFlags, ManagerClock, ManagerInputSink};

/// The game-thread side of the manager, once per `ClientTick`: the host tick (config writes, telemetry, the console
/// lines), the screen on top for the host frame, whether the player is in play, then the client flags. It does
/// not allocate.
pub struct TickService {
    sink: ManagerInputSink,
    clock: ManagerClock,
    flags: ClientFlags,
    // the screen code of the last tick, read by the render thread for the host frame
    menu: Arc<AtomicU8>,
}

impl TickService {
    /// Any screen other than the start or pause screen, and play.
    pub const MENU_NONE: u8 = 0;
    pub const MENU_START: u8 = 1;
    pub const MENU_PAUSE: u8 = 2;

    pub fn new(sink: ManagerInputSink, clock: ManagerClock, flags: ClientFlags, menu: Arc<AtomicU8>) -> Self {
        Self { sink, clock, flags, menu }
    }

    /// One `ClientTick`. With no client in the event (before the first client update, after a world leave) only
    /// the host ticks.
    pub fn on_tick(&mut self, at: &EventCtx) {
        let _ = HostCell::global().tick(self.clock.now_ms());
        let Some(client) = at.client() else {
            return;
        };
        self.menu.store(Self::menu_code(client.top_screen()), Ordering::Relaxed);
        let want = self.sink.wants(Self::in_play(&client));
        self.flags.apply(&client, want);
    }

    /// The host frame's code of a top screen: 1 the start screen, 2 the pause screen, 0 any other screen, play and
    /// an unread screen.
    pub fn menu_code(screen: Option<Screen>) -> u8 {
        match screen {
            Some(Screen::Start) => Self::MENU_START,
            Some(Screen::Pause) => Self::MENU_PAUSE,
            _ => Self::MENU_NONE,
        }
    }

    /// The player is in a world with no game screen on top: a local player, an active world and no menu.
    pub fn in_play(client: &Client<'_>) -> bool {
        client.local_player().is_some() && client.world_active() == Some(true) && client.showing_menu() != Some(true)
    }

    pub fn flags(&self) -> &ClientFlags {
        &self.flags
    }

    /// The screen code of the last tick.
    pub fn menu_screen(&self) -> u8 {
        self.menu.load(Ordering::Relaxed)
    }
}
