use std::ffi::c_void;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};

use windows::Win32::Graphics::Direct3D12::{D3D12_COMMAND_LIST_TYPE_DIRECT, ID3D12CommandQueue};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;
use windows::Win32::Graphics::Dxgi::{DXGI_PRESENT_TEST, IDXGISwapChain};
use windows::core::Interface;

use amm_ffi::services::PresentSequence;

use crate::frame_flow::FrameFlow;
use crate::in_flight::InFlight;
use crate::overlay_error::OverlayError;
use crate::present_pace::PresentPace;
use crate::queue_slot::QueueSlot;

static SHARED: OverlayShared = OverlayShared::new();

/// The process-wide state the detours reach: a detour has no context pointer, and the DXGI vtable it hooks is
/// shared by every chain of the process, so there is one overlay per process. The render side sits behind a lock
/// that only the render thread takes in a Present or ResizeBuffers, and shutdown after the drain.
pub(crate) struct OverlayShared {
    pub(crate) in_flight: InFlight,
    pub(crate) queue: QueueSlot,
    pub(crate) pace: PresentPace,
    // set from the start of shutdown; a Present that sees it draws nothing
    shutting_down: AtomicBool,
    // set after a panic in a detour; the detours then only call the original
    disabled: AtomicBool,
    panic_logged: AtomicBool,
    // an overlay holds the hooks, from install until a shutdown that drained
    installed: AtomicBool,
    // a shutdown timed out and left the render side alive; no new install in this process
    leaked: AtomicBool,
    // the bound chain's width in the high 32 bits and height in the low 32 bits, so a reader on another thread
    // never sees half of a resize; 0 until a chain is bound
    back_buffer_size: AtomicU64,
    // the bound chain's DXGI format value, written with the size; 0 (unknown) until a chain is bound
    back_buffer_format: AtomicU32,
    // the bound window's handle value; 0 until a chain is bound and after shutdown
    game_window: AtomicUsize,
    flow: Mutex<Option<FrameFlow>>,
}

impl OverlayShared {
    const WIDTH_SHIFT: u32 = 32;

    const fn new() -> Self {
        Self {
            in_flight: InFlight::new(),
            queue: QueueSlot::new(),
            pace: PresentPace::new(),
            shutting_down: AtomicBool::new(false),
            disabled: AtomicBool::new(false),
            panic_logged: AtomicBool::new(false),
            installed: AtomicBool::new(false),
            leaked: AtomicBool::new(false),
            back_buffer_size: AtomicU64::new(0),
            back_buffer_format: AtomicU32::new(0),
            game_window: AtomicUsize::new(0),
            flow: Mutex::new(None),
        }
    }

    pub(crate) fn global() -> &'static Self {
        &SHARED
    }

    /// Takes the one overlay of the process.
    pub(crate) fn claim(&self) -> Result<(), OverlayError> {
        if self.leaked.load(Ordering::SeqCst) {
            return Err(OverlayError::Leaked);
        }
        if self.installed.swap(true, Ordering::SeqCst) {
            return Err(OverlayError::AlreadyInstalled);
        }
        Ok(())
    }

    /// Gives the overlay up after an install that failed.
    pub(crate) fn unclaim(&self) {
        *self.lock_flow() = None;
        self.installed.store(false, Ordering::SeqCst);
    }

    /// Arms the detours with a fresh render side, before any hook is in place.
    pub(crate) fn start(&self, flow: FrameFlow) {
        self.shutting_down.store(false, Ordering::SeqCst);
        self.disabled.store(false, Ordering::SeqCst);
        self.panic_logged.store(false, Ordering::SeqCst);
        *self.lock_flow() = Some(flow);
    }

    pub(crate) fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::SeqCst);
    }

    /// Releases the render side after the drain; a later install may run.
    pub(crate) fn finish_shutdown(&self) {
        let flow = self.lock_flow().take();
        if let Some(mut flow) = flow {
            flow.release_binding(self);
        }
        self.queue.clear();
        self.installed.store(false, Ordering::SeqCst);
    }

    /// Keeps the render side alive for the thread still inside a detour.
    pub(crate) fn mark_leaked(&self) {
        self.leaked.store(true, Ordering::SeqCst);
    }

    pub(crate) fn shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::SeqCst)
    }

    pub(crate) fn record_size(&self, width: u32, height: u32, format: DXGI_FORMAT) {
        self.back_buffer_format.store(format.0 as u32, Ordering::Relaxed);
        let packed = (u64::from(width) << Self::WIDTH_SHIFT) | u64::from(height);
        self.back_buffer_size.store(packed, Ordering::Relaxed);
    }

    pub(crate) fn back_buffer_size(&self) -> (u32, u32) {
        let packed = self.back_buffer_size.load(Ordering::Relaxed);
        ((packed >> Self::WIDTH_SHIFT) as u32, packed as u32)
    }

    pub(crate) fn back_buffer_format(&self) -> DXGI_FORMAT {
        DXGI_FORMAT(self.back_buffer_format.load(Ordering::Relaxed) as i32)
    }

    pub(crate) fn set_game_window(&self, window: usize) {
        self.game_window.store(window, Ordering::SeqCst);
    }

    pub(crate) fn game_window(&self) -> usize {
        self.game_window.load(Ordering::SeqCst)
    }

    /// Runs `body` for detour `entry` and returns its value, or `fallback` when the overlay is off. A panic is
    /// contained as `contain` does.
    pub(crate) fn guard<T>(&self, entry: &'static str, fallback: T, body: impl FnOnce() -> T) -> T {
        if self.disabled.load(Ordering::SeqCst) {
            return fallback;
        }
        self.contain(entry, fallback, body)
    }

    /// Runs `body` for detour `entry` and returns its value, also while the overlay is off. A panic is caught: it
    /// is logged once, the overlay is switched off, and `fallback` is returned, so the detour still calls the
    /// original.
    pub(crate) fn contain<T>(&self, entry: &'static str, fallback: T, body: impl FnOnce() -> T) -> T {
        match std::panic::catch_unwind(AssertUnwindSafe(body)) {
            Ok(value) => value,
            Err(_) => {
                if !self.panic_logged.swap(true, Ordering::SeqCst) {
                    curia::error!("overlay detour panicked, the overlay is off for this session", { entry: entry });
                }
                self.switch_off();
                fallback
            }
        }
    }

    // after a panic no frame is drawn again, so the bound chain's targets go now: the game's next ResizeBuffers
    // fails while one of them holds a back buffer
    fn switch_off(&self) {
        self.disabled.store(true, Ordering::SeqCst);
        let released = std::panic::catch_unwind(AssertUnwindSafe(|| {
            if let Some(flow) = self.lock_flow().as_mut() {
                flow.switch_off();
            }
        }));
        if released.is_err() {
            curia::error!("overlay could not release its targets after a panic, a resize of the game window may fail");
        }
    }

    /// A Present of `chain` with `flags`. A test Present, and a Present that Present1 reached inside DXGI
    /// (`outermost` false), draw nothing.
    pub(crate) fn present(&self, chain: *mut c_void, flags: u32, outermost: bool) {
        if !outermost || flags & DXGI_PRESENT_TEST.0 != 0 || self.shutting_down() {
            return;
        }
        // the Present's number, which the host puts on the capture stream frames it offers in this Present
        PresentSequence::next();
        // SAFETY: DXGI calls Present with a live swap chain for the call
        let Some(chain) = (unsafe { IDXGISwapChain::from_raw_borrowed(&chain) }) else {
            return;
        };
        if let Some(flow) = self.lock_flow().as_mut() {
            flow.present(self, chain);
        }
    }

    /// Before a `ResizeBuffers` of `chain` asking `width` x `height`: `None` when it is not the bound chain, else
    /// the size to pass to the original, which is the forced size or the asked one.
    pub(crate) fn before_resize(&self, chain: *mut c_void, width: u32, height: u32) -> Option<(u32, u32)> {
        // SAFETY: DXGI calls ResizeBuffers with a live swap chain for the call
        let chain = unsafe { IDXGISwapChain::from_raw_borrowed(&chain) }?;
        self.lock_flow().as_mut().and_then(|flow| flow.before_resize(self, chain, width, height))
    }

    /// After a successful `ResizeBuffers` of the bound chain.
    pub(crate) fn after_resize(&self, chain: *mut c_void) {
        // SAFETY: as for before_resize
        let Some(chain) = (unsafe { IDXGISwapChain::from_raw_borrowed(&chain) }) else {
            return;
        };
        if let Some(flow) = self.lock_flow().as_ref() {
            flow.after_resize(self, chain);
        }
    }

    /// An `ExecuteCommandLists` on `queue`: the first DIRECT queue is kept for the D3D12 path.
    pub(crate) fn offer_queue(&self, queue: *mut c_void) {
        if self.shutting_down() || !self.queue.is_empty() {
            return;
        }
        // SAFETY: D3D12 calls ExecuteCommandLists with a live queue for the call
        let Some(queue) = (unsafe { ID3D12CommandQueue::from_raw_borrowed(&queue) }) else {
            return;
        };
        // SAFETY: the queue is live
        if unsafe { queue.GetDesc() }.Type == D3D12_COMMAND_LIST_TYPE_DIRECT {
            self.queue.offer(queue);
        }
    }

    // a panic while the lock was held switched the overlay off; the state is still released at shutdown
    fn lock_flow(&self) -> MutexGuard<'_, Option<FrameFlow>> {
        self.flow.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
