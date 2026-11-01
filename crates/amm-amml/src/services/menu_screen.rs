use std::sync::atomic::{AtomicU8, Ordering};

use crate::traits::GameCalls;

/// The game screen on top, for the host frame: 1 for the start screen, 2 for the pause screen, 0 for any other
/// screen or play. Written on the game thread, read on the render thread.
pub struct MenuScreen {
    current: AtomicU8,
}

impl MenuScreen {
    pub const NONE: u8 = 0;
    pub const START: u8 = 1;
    pub const PAUSE: u8 = 2;

    pub const fn new() -> Self {
        Self { current: AtomicU8::new(Self::NONE) }
    }

    /// The code of a top screen name.
    pub fn code(name: &[u8]) -> u8 {
        match name {
            b"start_screen" => Self::START,
            b"pause_screen" => Self::PAUSE,
            _ => Self::NONE,
        }
    }

    /// Once per tick: reads the top screen name, keeps its code and frees the string through the game heap.
    pub fn observe(&self, calls: &dyn GameCalls, client: usize) {
        let mut code = Self::NONE;
        let mut raw = String::new();
        if client != 0
            && let Some(name) = calls.top_screen_name(client)
        {
            code = Self::code(name.as_bytes());
            raw = String::from_utf8_lossy(name.as_bytes()).into_owned();
            calls.release_string(name);
        }
        let old = self.current.swap(code, Ordering::Relaxed);
        if old != code {
            curia::debug!("menu screen changed", { old: old, new: code, screen: raw });
        }
    }

    pub fn current(&self) -> u8 {
        self.current.load(Ordering::Relaxed)
    }
}

impl Default for MenuScreen {
    fn default() -> Self {
        Self::new()
    }
}
