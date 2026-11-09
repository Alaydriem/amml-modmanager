use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::{Duration, Instant};

use amm_abi::types::AmmCursorShape;
use amm_ffi::types::AmmHostPointerKind;
use amm_overlay::{InputSink, UninstallOutcome, WndProcRouter};

/// One `InputSink::pointer` call.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointerCall {
    pub kind: AmmHostPointerKind,
    pub x: f32,
    pub y: f32,
    pub button: u8,
    pub down: bool,
    pub wheel: f32,
}

/// An `InputSink` that records its calls and can consume, panic, block or uninstall the router on demand.
pub struct TestSink {
    pub consume: AtomicBool,
    pub panic_on_pointer: AtomicBool,
    pub cursor: Mutex<(bool, AmmCursorShape)>,
    // uninstall the router inside the next pointer call and keep the outcome
    pub uninstall_in_pointer: AtomicBool,
    pub uninstall_outcome: Mutex<Option<(UninstallOutcome, Duration)>>,
    // block the next pointer call until `release` is set
    pub block_pointer: AtomicBool,
    pub entered: AtomicBool,
    pub release: AtomicBool,
    pub pointers: Mutex<Vec<PointerCall>>,
    pub chars: Mutex<Vec<u32>>,
    pub focus_lost_calls: AtomicU32,
    // what `focus_lost` answers: true swallows the focus message
    pub swallow_focus: AtomicBool,
    pub window_changed_calls: AtomicU32,
    // the size `window_size` gives; none passes the message unchanged
    pub window_size: Mutex<Option<(u32, u32)>>,
    // every `window_size` call as (minimised, width, height)
    pub window_sizes: Mutex<Vec<(bool, u32, u32)>>,
}

impl TestSink {
    pub fn leaked() -> &'static TestSink {
        Box::leak(Box::new(Self {
            consume: AtomicBool::new(false),
            panic_on_pointer: AtomicBool::new(false),
            cursor: Mutex::new((false, AmmCursorShape::Default)),
            uninstall_in_pointer: AtomicBool::new(false),
            uninstall_outcome: Mutex::new(None),
            block_pointer: AtomicBool::new(false),
            entered: AtomicBool::new(false),
            release: AtomicBool::new(false),
            pointers: Mutex::new(Vec::new()),
            chars: Mutex::new(Vec::new()),
            focus_lost_calls: AtomicU32::new(0),
            swallow_focus: AtomicBool::new(false),
            window_changed_calls: AtomicU32::new(0),
            window_size: Mutex::new(None),
            window_sizes: Mutex::new(Vec::new()),
        }))
    }

    pub fn pointers(&self) -> Vec<PointerCall> {
        self.pointers.lock().unwrap().clone()
    }

    pub fn chars(&self) -> Vec<u32> {
        self.chars.lock().unwrap().clone()
    }
}

impl InputSink for TestSink {
    fn pointer(&self, kind: AmmHostPointerKind, x: f32, y: f32, button: u8, down: bool, wheel: f32) -> bool {
        self.pointers.lock().unwrap().push(PointerCall { kind, x, y, button, down, wheel });
        if self.panic_on_pointer.load(Ordering::SeqCst) {
            panic!("sink panic on purpose");
        }
        if self.uninstall_in_pointer.swap(false, Ordering::SeqCst) {
            let start = Instant::now();
            let outcome = WndProcRouter::uninstall();
            *self.uninstall_outcome.lock().unwrap() = Some((outcome, start.elapsed()));
        }
        if self.block_pointer.swap(false, Ordering::SeqCst) {
            self.entered.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                std::thread::sleep(Duration::from_millis(1));
            }
        }
        self.consume.load(Ordering::SeqCst)
    }

    fn char(&self, codepoint: u32) -> bool {
        self.chars.lock().unwrap().push(codepoint);
        self.consume.load(Ordering::SeqCst)
    }

    fn focus_lost(&self) -> bool {
        self.focus_lost_calls.fetch_add(1, Ordering::SeqCst);
        self.swallow_focus.load(Ordering::SeqCst)
    }

    fn window_size(&self, minimised: bool, width: u32, height: u32) -> Option<(u32, u32)> {
        self.window_sizes.lock().unwrap().push((minimised, width, height));
        *self.window_size.lock().unwrap()
    }

    fn window_changed(&self) {
        self.window_changed_calls.fetch_add(1, Ordering::SeqCst);
    }

    fn cursor(&self) -> (bool, AmmCursorShape) {
        *self.cursor.lock().unwrap()
    }
}
