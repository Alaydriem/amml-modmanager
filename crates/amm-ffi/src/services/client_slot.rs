use amm_abi::types::{AmmCallbacks, AmmFrameInput, AmmFrameOutput, AmmHandle};
use amm_core::structs::frame::{FrameInputData, LayerKind};
use amm_core::traits::AbiSized;

use super::UiEventBuffer;

/// The manager's side of one registration slot: the generation that tells a live handle from a stale one, the
/// mod's callbacks, the config text last handed out, and the routed events for the next frame of each of the mod's
/// two layers, Hud (index 0) and Overlay (index 1).
pub struct ClientSlot {
    generation: u16,
    callbacks: Option<AmmCallbacks>,
    document: String,
    events: [UiEventBuffer; LayerKind::COUNT],
}

// SAFETY: the callbacks hold the mod's `user` pointer, which the manager never reads. It is only passed back to the
// mod's own callbacks on the render thread under the host lock, as the `AmmCallbacks` contract allows.
unsafe impl Send for ClientSlot {}

impl ClientSlot {
    pub fn new(event_capacity: usize) -> Self {
        let events = [UiEventBuffer::new(event_capacity), UiEventBuffer::new(event_capacity)];
        Self { generation: 0, callbacks: None, document: String::new(), events }
    }

    /// Takes the slot for a new registration and returns its generation.
    pub fn occupy(&mut self, callbacks: AmmCallbacks) -> u16 {
        self.generation = self.generation.wrapping_add(1);
        self.callbacks = Some(callbacks);
        self.document.clear();
        self.clear_events();
        self.generation
    }

    pub fn vacate(&mut self) {
        self.callbacks = None;
        self.document = String::new();
        self.clear_events();
    }

    /// True when the handle names this slot's live registration.
    pub fn matches(&self, handle: AmmHandle) -> bool {
        self.callbacks.is_some() && self.generation == handle.generation
    }

    pub fn is_live(&self) -> bool {
        self.callbacks.is_some()
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

    /// Calls the mod's frame callback for the layer `kind` with `base` and the routed events of that layer, then
    /// clears them. Returns the status the callback gave and what it filled in, or `None` when the mod has no frame
    /// callback.
    pub fn run_frame(&mut self, base: &FrameInputData, kind: LayerKind) -> Option<(i32, AmmFrameOutput)> {
        let callbacks = self.callbacks.as_ref()?;
        let frame = callbacks.frame?;
        let events = &mut self.events[kind.slot()];
        let data = FrameInputData { events: events.as_slice(), ..*base };
        let input = AmmFrameInput::from(&data);
        let mut out = Self::blank_output();
        // SAFETY: the mod gave this callback and user at register and keeps both valid until unregister returns,
        // which cannot happen while the host lock is held; input and its events live for the call
        let status = unsafe { frame(callbacks.user, &input, &mut out) };
        events.clear();
        Some((status, out))
    }

    /// Tells the mod that the host lost every texture.
    pub fn textures_lost(&self) {
        let Some(callbacks) = self.callbacks.as_ref() else {
            return;
        };
        if let Some(textures_lost) = callbacks.textures_lost {
            // SAFETY: the mod gave this callback and user at register and keeps both valid until unregister
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

    fn blank_output() -> AmmFrameOutput {
        AmmFrameOutput::blank()
    }
}
