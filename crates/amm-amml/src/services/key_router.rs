use amm_core::structs::input::{InputEvent, Modifiers as HostModifiers};
use amml_api::{KeyCode, Modifiers};

use super::{ManagerClock, ManagerInputSink};
use crate::types::KeyRoute;

/// The keys of the game's key feed, to the manager host. The host matches the chord of each press against the
/// manager chord (which opens and closes the panel) and the key binds of the mods it hosts (whose `Keybind`
/// events go to those mods). Modifiers are compared with `generic()`, so a left or right Ctrl matches a `Ctrl+...`
/// bind; F3 comes from `Input::modifiers()`, because a `KeyInput` never carries it. A press another listener
/// cancelled still reaches the open panel and the manager chord, never a bind. The repeats and the up of a key go
/// to the host only when its down went there. `on_key` does not allocate.
pub struct KeyRouter {
    sink: ManagerInputSink,
    clock: ManagerClock,
    // per virtual key: its down went to the host, so its repeats and its up go there too
    routed: [bool; InputEvent::VK_SLOTS],
    // the host was told that F3 is down
    f3_routed: bool,
}

impl KeyRouter {
    pub fn new(sink: ManagerInputSink, clock: ManagerClock) -> Self {
        Self { sink, clock, routed: [false; InputEvent::VK_SLOTS], f3_routed: false }
    }

    /// One real key event. `modifiers` are the event's modifiers with F3 from `Input::modifiers()`; `cancelled` is
    /// true when a listener before this one (or a lease block) cancelled it.
    pub fn on_key(
        &mut self,
        key: KeyCode,
        down: bool,
        repeat: bool,
        modifiers: Modifiers,
        cancelled: bool,
    ) -> KeyRoute {
        let vk = key.generic();
        let slot = usize::from(vk.0);
        if slot >= InputEvent::VK_SLOTS {
            return KeyRoute::PASS;
        }
        let held = modifiers.generic();
        let mods = HostModifiers { ctrl: held.ctrl(), shift: held.shift(), alt: held.alt() };
        let now_ms = self.clock.now_ms();
        if vk != KeyCode::F3 {
            self.sync_f3(held.f3(), mods, now_ms);
        }
        let press = down && !(repeat && self.routed[slot]);
        if press {
            if cancelled && !self.sink.takes_cancelled(u32::from(vk.0), mods, held.f3()) {
                self.routed[slot] = false;
                return KeyRoute::PASS;
            }
        } else if !self.routed[slot] {
            return KeyRoute::PASS;
        }
        let consume =
            self.sink.route(InputEvent::Key { vk: u32::from(vk.0), down, repeat: repeat && !press, mods }, now_ms);
        self.routed[slot] = down;
        if vk == KeyCode::F3 {
            self.f3_routed = down;
        }
        KeyRoute { consume }
    }

    // Tells the host about F3 when `Input::modifiers()` and what the host was told differ: F3 went down or up while
    // its own event did not reach this router (before the manager listened, or cancelled with the panel closed).
    fn sync_f3(&mut self, f3: bool, mods: HostModifiers, now_ms: u64) {
        if f3 == self.f3_routed {
            return;
        }
        let vk = u32::from(KeyCode::F3.0);
        self.sink.route(InputEvent::Key { vk, down: f3, repeat: false, mods }, now_ms);
        self.f3_routed = f3;
        self.routed[usize::from(KeyCode::F3.0)] = f3;
    }
}
