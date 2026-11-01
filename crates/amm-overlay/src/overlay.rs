use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use windows::Win32::Foundation::HWND;

use crate::dxgi_probe::DxgiProbe;
use crate::frame_flow::FrameFlow;
use crate::hook_installer::HookInstaller;
use crate::hook_token::HookToken;
use crate::input_sink::InputSink;
use crate::overlay_error::OverlayError;
use crate::overlay_host::OverlayHost;
use crate::overlay_shared::OverlayShared;
use crate::present_hooks::PresentHooks;
use crate::shutdown_outcome::ShutdownOutcome;
use crate::wndproc::WndProcRouter;

/// The overlay on the game's swap chain: hooks DXGI Present, Present1, ResizeBuffers and ResizeBuffers1, and the
/// D3D12 ExecuteCommandLists, and draws the host's layers on the back buffer before each Present. Works on D3D11
/// and on D3D12 (through D3D11On12). One per process. The device, the targets and the layer renderer are made on
/// the first Present of the bound chain.
pub struct Overlay<I: HookInstaller> {
    installer: I,
    // at most five, one per hooked entry
    tokens: Mutex<Vec<HookToken>>,
    outcome: Mutex<Option<ShutdownOutcome>>,
}

impl<I: HookInstaller> Overlay<I> {
    /// How long the game's shutdown waits for a thread inside a detour.
    pub const SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(2000);

    /// Probes the entries and hooks them through `installer`. Refused while another overlay of the process is
    /// installed, and after a shutdown that leaked. With an `input` sink, the game window is subclassed when the
    /// overlay binds to it (`WndProcRouter::install`); `WndProcRouter::uninstall` is a separate call, which comes
    /// before `shutdown`. After that uninstall no bind of this overlay subclasses the window again, also not a
    /// device rebuild; the next `install` allows it again.
    pub fn install(
        installer: I,
        host: &'static dyn OverlayHost,
        input: Option<&'static dyn InputSink>,
    ) -> Result<Self, OverlayError> {
        let shared = OverlayShared::global();
        shared.claim()?;
        WndProcRouter::open();
        let tokens = DxgiProbe::targets().and_then(|targets| {
            shared.start(FrameFlow::new(host, input));
            PresentHooks::install(&installer, &targets)
        });
        match tokens {
            Ok(tokens) => {
                curia::info!("overlay swap chain hooks installed");
                Ok(Self { installer, tokens: Mutex::new(tokens), outcome: Mutex::new(None) })
            }
            Err(error) => {
                curia::warn!("overlay could not hook the swap chain, no layers are drawn this session", {
                    error: error.to_string()
                });
                shared.unclaim();
                Err(error)
            }
        }
    }

    /// Stops new frames, removes the hooks and waits up to `timeout` for the threads inside a detour. With none
    /// left, releases the device, the targets and the layer textures (`Drained`). Past the timeout it frees
    /// nothing, since a thread inside a detour can still use it (`Leaked`). A second call returns the first
    /// outcome.
    pub fn shutdown(&self, timeout: Duration) -> ShutdownOutcome {
        let mut outcome = self.outcome.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(outcome) = *outcome {
            return outcome;
        }
        let shared = OverlayShared::global();
        shared.begin_shutdown();
        PresentHooks::uninstall(&self.installer, &mut self.tokens.lock().unwrap_or_else(PoisonError::into_inner));
        let result = if shared.in_flight.wait_drained(timeout) {
            shared.finish_shutdown();
            ShutdownOutcome::Drained
        } else {
            shared.mark_leaked();
            curia::warn!("overlay leaves its device alive, a thread is still inside a swap chain hook");
            ShutdownOutcome::Leaked
        };
        *outcome = Some(result);
        result
    }

    /// The bound chain's back-buffer size in pixels, read on any thread; (0, 0) until a chain is bound.
    pub fn back_buffer_size() -> (u32, u32) {
        OverlayShared::global().back_buffer_size()
    }

    /// The window of the bound chain, read on any thread; `None` until a chain is bound and after shutdown.
    pub fn game_window() -> Option<HWND> {
        match OverlayShared::global().game_window() {
            0 => None,
            window => Some(HWND(window as *mut _)),
        }
    }
}
