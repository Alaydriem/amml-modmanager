use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

use amm_core::structs::render::RenderSize;
use amm_ffi::services::HostCell;
use amml_api::amml_abi::{SwapChainResize, SwapChainResized, WindowSize};
use amml_api::{Event, Logger, Size, SwapChainResizeExt, Window, WindowSizeExt};

/// The render size a `RenderSize` holder forces, applied through the loader: every resize of the bound swap chain
/// and every real `WM_SIZE` of the game window take the forced size (`WRITE` listeners on `SwapChainResize` and
/// `WindowSize`), and the size messages the host frames give go to the game window through
/// `Window::request_size`: `Some(size)` while a size is forced, `None` (the window's own size) when it ended. A
/// `WM_SIZE` the loader posted (`request_size`, a give-back, a restore) is raised as injected and left alone, so
/// a posted size never loops. The forced size is read from the host's lock-free state, so the render and window
/// threads never wait on the host lock. No allocation outside a log line.
pub struct RenderSizeControl {
    cell: &'static HostCell,
    window: Window,
    log: Logger,
    // the last request sent: `width << 32 | height`, `GIVEN_BACK` for `None`, 0 before the first
    last_request: AtomicU64,
    // a refused request was logged; cleared by the next request the loader takes
    refusal_logged: AtomicBool,
}

impl RenderSizeControl {
    const GIVEN_BACK: u64 = u64::MAX;

    pub fn new(cell: &'static HostCell, window: Window, log: Logger) -> Self {
        Self { cell, window, log, last_request: AtomicU64::new(0), refusal_logged: AtomicBool::new(false) }
    }

    /// The size a `RenderSize` holder forces now, if any.
    pub fn forced(&self) -> Option<RenderSize> {
        let (forced, size) = self.cell.render_size_for(RenderSize::default());
        forced.then_some(size)
    }

    /// `SwapChainResize`, in `HookOrder::WRITE`: the chain takes the forced size.
    pub fn on_resize(&self, event: &mut SwapChainResize) {
        let Some(forced) = self.forced() else {
            return;
        };
        let size = Size { width: forced.width, height: forced.height };
        if event.size() != size {
            event.set_size(size);
        }
    }

    /// `SwapChainResized`: one line per resize, for the in-game checks.
    pub fn on_resized(&self, event: &SwapChainResized) {
        let ok = if event.ok != 0 { "true" } else { "false" };
        let (width, height) = (event.width.to_string(), event.height.to_string());
        self.log.info("resized", &[("ok", ok), ("width", &width), ("height", &height)]);
    }

    /// `WindowSize`, in `HookOrder::WRITE`: a real size message gives the game the forced size, so a resize of the
    /// window keeps it; a minimised window and a size the loader posted pass unchanged.
    pub fn on_window_size(&self, event: &mut WindowSize) {
        self.cell.forget_client_size();
        if event.is_injected() {
            return;
        }
        let asked = event.size();
        let (forced, size) = self.cell.window_size_for(event.minimised(), RenderSize::new(asked.width, asked.height));
        if forced {
            event.set_size(Size { width: size.width, height: size.height });
        }
    }

    /// `WindowChanged`: a DPI, display or position change; while a size is forced the next frame sends it again.
    pub fn on_window_changed(&self) {
        self.cell.window_changed();
    }

    /// Sends the size message of a host frame to the game window: the forced size, or the window's own size when
    /// the message is the give-back after a forced size ended. A request that changes is logged; the keep-alive
    /// resends of the same size are not.
    pub fn send(&self, message: RenderSize) {
        let request = self.forced().filter(|forced| *forced == message);
        let key = request.map_or(Self::GIVEN_BACK, |size| (u64::from(size.width) << 32) | u64::from(size.height));
        match self.window.request_size(request.map(|size| Size { width: size.width, height: size.height })) {
            Ok(()) => {
                self.refusal_logged.store(false, Ordering::Relaxed);
                if self.last_request.swap(key, Ordering::Relaxed) != key {
                    self.log_request(request);
                }
            }
            Err(error) => {
                if !self.refusal_logged.swap(true, Ordering::Relaxed) {
                    self.log.warn("render size request refused", &[("error", &error.to_string())]);
                }
            }
        }
    }

    fn log_request(&self, request: Option<RenderSize>) {
        match request {
            Some(size) => {
                let (width, height) = (size.width.to_string(), size.height.to_string());
                self.log.info("render size requested", &[("width", &width), ("height", &height)]);
            }
            None => self.log.info("render size given back", &[]),
        }
    }
}
