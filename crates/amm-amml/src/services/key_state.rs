use std::sync::atomic::{AtomicU64, Ordering};

use crate::traits::GameCalls;

/// The keys whose down went to the host and whose up has not, so a second down is a repeat. Left and right
/// modifier keys count as the plain key, so a chord such as "Ctrl" matches either side. Input thread.
pub struct KeyState {
    // 256 bits, one per virtual-key code
    held: [AtomicU64; 4],
}

impl KeyState {
    const VK_SHIFT: u32 = 0x10;
    const VK_CONTROL: u32 = 0x11;
    const VK_MENU: u32 = 0x12;
    const VK_LSHIFT: u32 = 0xA0;
    const VK_RSHIFT: u32 = 0xA1;
    const VK_LCONTROL: u32 = 0xA2;
    const VK_RCONTROL: u32 = 0xA3;
    const VK_LMENU: u32 = 0xA4;
    const VK_RMENU: u32 = 0xA5;
    const VK_COUNT: u32 = 256;

    pub const fn new() -> Self {
        Self { held: [const { AtomicU64::new(0) }; 4] }
    }

    /// The plain key of a left or right modifier key; any other key as it is.
    pub fn normalise(vk: u32) -> u32 {
        match vk {
            Self::VK_LSHIFT | Self::VK_RSHIFT => Self::VK_SHIFT,
            Self::VK_LCONTROL | Self::VK_RCONTROL => Self::VK_CONTROL,
            Self::VK_LMENU | Self::VK_RMENU => Self::VK_MENU,
            other => other,
        }
    }

    /// A key from the game's keyboard feed. True when the host consumed it, so the game must not see it.
    pub fn route(&self, calls: &dyn GameCalls, key: i32, down: bool) -> bool {
        let vk = Self::normalise(key as u32);
        let repeat = self.note(calls, vk, down);
        calls.host_key(vk, down, repeat, calls.modifiers(), calls.now_ms())
    }

    /// Forgets every held key.
    pub fn clear(&self) {
        for word in &self.held {
            word.store(0, Ordering::Relaxed);
        }
    }

    // a focus loss released every key, so a key held before it is not held now
    fn note(&self, calls: &dyn GameCalls, vk: u32, down: bool) -> bool {
        if calls.take_focus_lost() {
            self.clear();
        }
        if vk >= Self::VK_COUNT {
            return false;
        }
        let word = &self.held[(vk / 64) as usize];
        let bit = 1u64 << (vk % 64);
        let before = if down { word.fetch_or(bit, Ordering::Relaxed) } else { word.fetch_and(!bit, Ordering::Relaxed) };
        down && before & bit != 0
    }
}

impl Default for KeyState {
    fn default() -> Self {
        Self::new()
    }
}
