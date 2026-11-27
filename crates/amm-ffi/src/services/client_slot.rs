use amm_abi::types::{AmmCallbacks, AmmHandle};
use amm_core::structs::frame::LayerKind;

use super::UiEventBuffer;

/// The manager's side of one registration slot: whether it is taken, the generation that tells a live handle from a
/// stale one, the mod's callbacks (none until it sets them), the config text last handed out, and the routed events
/// for the next frame of each of the mod's two layers, Hud (index 0) and Overlay (index 1).
pub struct ClientSlot {
    live: bool,
    generation: u16,
    callbacks: Option<AmmCallbacks>,
    document: String,
    events: [UiEventBuffer; LayerKind::COUNT],
}

// SAFETY: the callbacks hold the mod's `user` pointer, which the manager never reads. It is only passed back to the
// mod's own callbacks on the render thread, while `unregister` and `set_callbacks` of the mod cannot return, as the
// `AmmCallbacks` contract allows.
unsafe impl Send for ClientSlot {}

impl ClientSlot {
    pub fn new(event_capacity: usize) -> Self {
        let events = [UiEventBuffer::new(event_capacity), UiEventBuffer::new(event_capacity)];
        Self { live: false, generation: 0, callbacks: None, document: String::new(), events }
    }

    /// Takes the slot for a new registration, with no callbacks, and returns its generation.
    pub fn occupy(&mut self) -> u16 {
        self.generation = self.generation.wrapping_add(1);
        self.live = true;
        self.callbacks = None;
        self.document.clear();
        self.clear_events();
        self.generation
    }

    /// Replaces the mod's callbacks.
    pub fn set_callbacks(&mut self, callbacks: AmmCallbacks) {
        self.callbacks = Some(callbacks);
    }

    pub fn vacate(&mut self) {
        self.live = false;
        self.callbacks = None;
        self.document = String::new();
        self.clear_events();
    }

    /// True when the handle names this slot's live registration.
    pub fn matches(&self, handle: AmmHandle) -> bool {
        self.live && self.generation == handle.generation
    }

    pub fn is_live(&self) -> bool {
        self.live
    }

    pub fn generation(&self) -> u16 {
        self.generation
    }

    pub fn callbacks(&self) -> Option<&AmmCallbacks> {
        self.callbacks.as_ref()
    }

    /// Keeps `text` as the document handed to the mod, valid until the next call.
    pub fn set_document(&mut self, text: String) -> &str {
        self.document = text;
        &self.document
    }

    /// The mod's callbacks when it set a frame callback.
    pub fn frame_callbacks(&self) -> Option<AmmCallbacks> {
        self.callbacks.filter(|callbacks| callbacks.frame.is_some())
    }

    /// Moves the routed events of the layer `kind` into `into`, and leaves the slot `into`'s old, cleared storage.
    /// Neither side allocates.
    pub fn take_events(&mut self, kind: LayerKind, into: &mut UiEventBuffer) {
        into.clear();
        std::mem::swap(&mut self.events[kind.slot()], into);
    }

    /// Tells the mod that the host lost every texture.
    pub fn textures_lost(&self) {
        let Some(callbacks) = self.callbacks.as_ref() else {
            return;
        };
        if let Some(textures_lost) = callbacks.textures_lost {
            // SAFETY: the mod gave this callback and user with set_callbacks and keeps both valid until unregister
            // returns, which cannot happen while the host lock is held
            unsafe { textures_lost(callbacks.user) };
        }
    }

    /// The routed events for the next frame of the layer `kind`.
    pub fn events_mut(&mut self, kind: LayerKind) -> &mut UiEventBuffer {
        &mut self.events[kind.slot()]
    }

    pub fn clear_events(&mut self) {
        for events in self.events.iter_mut() {
            events.clear();
        }
    }
}
