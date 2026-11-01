use std::cell::Cell;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use amm_abi::types::{AmmFrameInput, AmmStatus, AmmUiEvent, AmmUiEventKind};
use amm_client::bridge::{ClipboardWorker, EguiFrame};
use amm_client::link::FrameHandler;
use amm_core::structs::frame::{FrameInputData, LayerKind, UiEvent};

use super::{FakeTable, FocusAction};

/// Drives Overlay passes of an `EguiFrame` with keyboard events, and waits on the worker thread of a clipboard.
pub struct ClipboardPass;

impl ClipboardPass {
    const VK_CONTROL: u32 = 0x11;
    const VK_A: u32 = 0x41;
    const VK_C: u32 = 0x43;
    const VK_V: u32 = 0x56;
    const WAIT: Duration = Duration::from_secs(5);
    const POLL: Duration = Duration::from_millis(5);

    /// Runs one Overlay pass with keyboard and pointer allowed.
    pub fn run<H: FrameHandler>(frame: &mut EguiFrame<H>, events: &[AmmUiEvent]) {
        let input = AmmFrameInput {
            size: size_of::<AmmFrameInput>() as u32,
            reserved: [0; 6],
            width: 640,
            height: 480,
            pixels_per_point: 1.0,
            time_secs: 0.0,
            events: if events.is_empty() { std::ptr::null() } else { events.as_ptr() },
            event_count: events.len(),
            allowed_layers: FrameInputData::LAYER_OVERLAY,
            input_flags: 7,
        };
        let mut out = FakeTable::empty_output();
        // SAFETY: events lives for the call and event_count is its length
        let status = unsafe { frame.run(&input, &mut out) };
        assert_eq!(status, AmmStatus::Ok);
    }

    /// Shows the field, then gives it focus.
    pub fn focus<H: FrameHandler>(frame: &mut EguiFrame<H>, action: &Cell<FocusAction>) {
        Self::run(frame, &[]);
        action.set(FocusAction::Request);
        Self::run(frame, &[]);
        Self::run(frame, &[]);
    }

    pub fn select_all<H: FrameHandler>(frame: &mut EguiFrame<H>) {
        Self::run(frame, &Self::chord(Self::VK_A));
    }

    pub fn ctrl_c<H: FrameHandler>(frame: &mut EguiFrame<H>) {
        Self::run(frame, &Self::chord(Self::VK_C));
    }

    pub fn ctrl_v<H: FrameHandler>(frame: &mut EguiFrame<H>) {
        Self::run(frame, &Self::chord(Self::VK_V));
    }

    /// Waits until the memory clipboard holds `expected`. Returns false when it does not within the wait.
    pub fn wait_for_text(memory: &Arc<Mutex<String>>, expected: &str) -> bool {
        Self::wait_until(|| memory.lock().is_ok_and(|text| text.as_str() == expected))
    }

    /// Waits for the paste result of `layer`.
    pub fn wait_for_paste(worker: &ClipboardWorker, layer: LayerKind) -> Option<String> {
        let mut found = None;
        Self::wait_until(|| {
            found = worker.take_paste(layer);
            found.is_some()
        });
        found
    }

    /// Waits until the worker took every queued request.
    pub fn wait_for_empty_queue(worker: &ClipboardWorker) -> bool {
        Self::wait_until(|| worker.pending() == 0)
    }

    /// Runs passes until `done` holds. Returns false when it does not within the wait.
    pub fn run_until<H: FrameHandler>(frame: &mut EguiFrame<H>, done: impl Fn() -> bool) -> bool {
        Self::wait_until(|| {
            Self::run(frame, &[]);
            done()
        })
    }

    fn wait_until(mut done: impl FnMut() -> bool) -> bool {
        let start = Instant::now();
        while start.elapsed() < Self::WAIT {
            if done() {
                return true;
            }
            std::thread::sleep(Self::POLL);
        }
        false
    }

    // Ctrl down, the key down with Ctrl held, the key up, Ctrl up
    fn chord(vk: u32) -> [AmmUiEvent; 4] {
        let ctrl = UiEvent::MOD_CTRL;
        [
            Self::key(Self::VK_CONTROL, true, ctrl),
            Self::key(vk, true, ctrl),
            Self::key(vk, false, ctrl),
            Self::key(Self::VK_CONTROL, false, 0),
        ]
    }

    fn key(key: u32, down: bool, modifiers: u8) -> AmmUiEvent {
        AmmUiEvent { kind: AmmUiEventKind::Key, x: 0.0, y: 0.0, key, modifiers, down, ch: 0 }
    }
}
