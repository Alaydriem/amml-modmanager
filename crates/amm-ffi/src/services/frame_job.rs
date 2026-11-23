use amm_abi::types::{AmmCallbacks, AmmFrameInput, AmmFrameOutput};
use amm_core::structs::frame::FrameInputData;
use amm_core::structs::snapshot::LayerEntry;
use amm_core::traits::AbiSized;

use super::{JobOutcome, UiEventBuffer};
use crate::types::AmmHostFrameInput;

/// One layer's frame callback of one compose, taken under the host lock and run without it: the layer, a copy of
/// the mod's callbacks, its input flags, whether the mod may draw the game picture, the events routed to the layer
/// (moved out of the mod's slot, so routing goes on into the slot while the callback runs), and what it came to.
/// The event storage is reserved at construction and swapped with the slot's, so a compose does not allocate.
pub struct FrameJob {
    entry: Option<LayerEntry>,
    callbacks: Option<AmmCallbacks>,
    flags: u8,
    picture: bool,
    events: UiEventBuffer,
    outcome: JobOutcome,
}

// SAFETY: the callbacks hold the mod's `user` pointer, which the manager never reads. It is only passed back to the
// mod's own frame callback on the render thread, while the host's frame gate keeps `unregister` and
// `set_callbacks` of that mod from returning, as the `AmmCallbacks` contract allows.
unsafe impl Send for FrameJob {}

impl FrameJob {
    pub fn new(event_capacity: usize) -> Self {
        Self {
            entry: None,
            callbacks: None,
            flags: 0,
            picture: false,
            events: UiEventBuffer::new(event_capacity),
            outcome: JobOutcome::NotRun,
        }
    }

    /// Readies the job for the layer `entry` with the mod's `callbacks`, input `flags`, and whether it may draw the
    /// game picture. The routed events are swapped in through `events_mut`.
    pub fn prepare(&mut self, entry: LayerEntry, callbacks: AmmCallbacks, flags: u8, picture: bool) {
        self.entry = Some(entry);
        self.callbacks = Some(callbacks);
        self.flags = flags;
        self.picture = picture;
        self.events.clear();
        self.outcome = JobOutcome::NotRun;
    }

    /// The job's event buffer, to swap with the slot's buffer of the layer.
    pub fn events_mut(&mut self) -> &mut UiEventBuffer {
        &mut self.events
    }

    pub fn entry(&self) -> Option<LayerEntry> {
        self.entry
    }

    /// Whether the mod held `Overlay`, and so may draw the game picture, when the compose began.
    pub fn picture(&self) -> bool {
        self.picture
    }

    /// Calls the frame callback with the layer's events, then clears them, and gives the status and output the
    /// callback filled in. `None` for a mod with no frame callback.
    ///
    /// # Safety
    ///
    /// The callbacks were set by the mod and stay valid for the call: no `unregister` or `set_callbacks` of the mod
    /// returns while it runs.
    pub unsafe fn call(&mut self, input: &AmmHostFrameInput) -> Option<(i32, AmmFrameOutput)> {
        let (entry, callbacks) = (self.entry?, self.callbacks?);
        let frame = callbacks.frame?;
        let data = FrameInputData {
            width: input.width,
            height: input.height,
            pixels_per_point: input.pixels_per_point,
            time_secs: input.time_secs,
            events: self.events.as_slice(),
            allowed_layers: entry.allowed,
            input_flags: self.flags,
        };
        let frame_input = AmmFrameInput::from(&data);
        let mut out = AmmFrameOutput::blank();
        // SAFETY: the caller keeps the callback and its user valid for the call; the input and its events live for
        // the call
        let status = unsafe { frame(callbacks.user, &frame_input, &mut out) };
        self.events.clear();
        Some((status, out))
    }

    pub fn set_outcome(&mut self, outcome: JobOutcome) {
        self.outcome = outcome;
    }

    /// What the job came to, once; the job is then free for the next compose.
    pub fn take_outcome(&mut self) -> JobOutcome {
        self.entry = None;
        self.callbacks = None;
        std::mem::replace(&mut self.outcome, JobOutcome::NotRun)
    }
}
