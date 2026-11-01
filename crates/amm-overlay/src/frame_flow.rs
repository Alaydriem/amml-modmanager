use amm_ffi::types::{AMM_HOST_MAX_CAPTURES, AMM_HOST_MAX_STREAM_FRAMES, AmmHostFrame, AmmHostFrameInput};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Direct3D11::{ID3D11Device, ID3D11RenderTargetView, ID3D11Resource};
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_UNKNOWN;
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, IDXGISwapChain3};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::GetClientRect;
use windows::core::Interface;

use crate::capture::FrameCapture;
use crate::capture_kind::CaptureKind;
use crate::chain_targets::ChainTargets;
use crate::device::OverlayDevice;
use crate::device_start::DeviceStart;
use crate::draw_pass::DrawPass;
use crate::host_capture_sink::HostCaptureSink;
use crate::host_stream_sink::HostStreamSink;
use crate::input_sink::InputSink;
use crate::layer_renderer::LayerRenderer;
use crate::overlay_host::OverlayHost;
use crate::overlay_shared::OverlayShared;
use crate::size_message::SizeMessage;
use crate::wndproc::WndProcRouter;

/// The render side of the overlay: which chain it is bound to, the device, the targets, the layer renderer and the
/// captures, and the flow of one Present. Lives in `OverlayShared` behind its lock; used inside Present and
/// ResizeBuffers, which the game never runs at the same time for the bound chain.
///
/// Binding: nothing is bound before the host says the game is ready. The first chain presented after that is
/// bound, and its window kept; a chain on another window is ignored. A new device on the bound window rebuilds the
/// device, the targets and the layer textures.
pub(crate) struct FrameFlow {
    host: &'static dyn OverlayHost,
    // where the bound window's input goes; none leaves the window alone
    input: Option<&'static dyn InputSink>,
    device: Option<OverlayDevice>,
    // the device could not be made; tried again only after a new device
    device_failed: bool,
    targets: ChainTargets,
    renderer: Option<LayerRenderer>,
    // the renderer could not start on this device; tried again only on a new device
    renderer_failed: bool,
    capture: FrameCapture<AMM_HOST_MAX_CAPTURES>,
    stream: FrameCapture<AMM_HOST_MAX_STREAM_FRAMES>,
    // the layer textures were dropped; the host is told before the next frame
    textures_lost_pending: bool,
    // the window drawn on, as its handle value, kept across a device rebuild
    bound_window: Option<usize>,
    // identity only: the device of the bound chain while `device` is set
    bound_device: usize,
    queue_wait_frames: u32,
    queue_wait_logged: bool,
    render_thread_logged: bool,
    ticks_per_second: f64,
}

// SAFETY: a FrameFlow lives only inside the lock of OverlayShared, so one thread at a time uses it. The D3D11 and
// D3D12 objects it holds are free-threaded, apart from the immediate context, which is used only under that lock,
// and the host is Sync
unsafe impl Send for FrameFlow {}

impl FrameFlow {
    const QUEUE_WAIT_LOG_FRAMES: u32 = 600;
    const DEFAULT_DPI: f32 = 96.0;

    pub(crate) fn new(host: &'static dyn OverlayHost, input: Option<&'static dyn InputSink>) -> Self {
        let mut frequency = 0i64;
        // SAFETY: the out pointer is a writable local
        let ticks_per_second = match unsafe { QueryPerformanceFrequency(&mut frequency) } {
            Ok(()) if frequency > 0 => frequency as f64,
            _ => 1.0,
        };
        Self {
            host,
            input,
            device: None,
            device_failed: false,
            targets: ChainTargets::new(),
            renderer: None,
            renderer_failed: false,
            capture: FrameCapture::new(CaptureKind::Screenshot),
            stream: FrameCapture::new(CaptureKind::Stream),
            textures_lost_pending: false,
            bound_window: None,
            bound_device: 0,
            queue_wait_frames: 0,
            queue_wait_logged: false,
            render_thread_logged: false,
            ticks_per_second,
        }
    }

    /// The outermost Present of `chain`, not a test Present. Binds the chain when it may, then asks the host for
    /// a frame and draws it.
    pub(crate) fn present(&mut self, shared: &OverlayShared, chain: &IDXGISwapChain) {
        if self.bound_window.is_none() && !self.host.game_ready() {
            return;
        }
        if !self.accept_chain(shared, chain) || shared.shutting_down() {
            return;
        }
        // the pace log follows the forced size itself, also when the game takes no ResizeBuffers for a change
        shared.pace.set_forced(self.host.size_forced());
        self.render(shared, chain);
    }

    /// Before `ResizeBuffers` of `chain` asking `width` x `height`: when it is the bound chain, drops the targets
    /// and the layer textures, and gives the size to pass on, which is the host's forced size unless the overlay
    /// is shutting down. `None` when it is not the bound chain.
    pub(crate) fn before_resize(
        &mut self,
        shared: &OverlayShared,
        chain: &IDXGISwapChain,
        width: u32,
        height: u32,
    ) -> Option<(u32, u32)> {
        if !self.targets.is_chain(chain) {
            return None;
        }
        self.release_targets();
        self.release_layer_textures();
        if shared.shutting_down() {
            shared.pace.set_forced(false);
            return Some((width, height));
        }
        let forced = self.host.render_size(width, height);
        shared.pace.set_forced(forced.is_some());
        let Some((forced_width, forced_height)) = forced else {
            return Some((width, height));
        };
        if (forced_width, forced_height) != (width, height) {
            curia::info!("game resize takes the forced render size", {
                asked_width: width,
                asked_height: height,
                width: forced_width,
                height: forced_height
            });
        }
        Some((forced_width, forced_height))
    }

    /// After a successful `ResizeBuffers` of the bound chain: the size it chose, since its arguments may be 0.
    pub(crate) fn after_resize(&self, shared: &OverlayShared, chain: &IDXGISwapChain) {
        // SAFETY: the chain is live for the ResizeBuffers that called this
        if let Ok(desc) = unsafe { chain.GetDesc() } {
            shared.record_size(desc.BufferDesc.Width, desc.BufferDesc.Height, desc.BufferDesc.Format);
        }
    }

    /// The overlay is off after a panic: drops the targets, so no back buffer stays referenced for the game's next
    /// ResizeBuffers.
    pub(crate) fn switch_off(&mut self) {
        self.release_targets();
    }

    /// Drops everything and forgets the window, after the drain of a shutdown.
    pub(crate) fn release_binding(&mut self, shared: &OverlayShared) {
        self.release_device(shared);
        self.bound_window = None;
        shared.set_game_window(0);
        shared.record_size(0, 0, DXGI_FORMAT_UNKNOWN);
    }

    fn render(&mut self, shared: &OverlayShared, chain: &IDXGISwapChain) {
        let index = self.back_buffer_index(chain);
        let ready = self.renderer.is_some() && self.host.game_ready();
        let target = index.and_then(|index| self.targets.target(index));
        let (Some(device), Some(renderer), Some((view, wrapped)), true) =
            (self.device.as_ref(), self.renderer.as_mut(), target, ready)
        else {
            self.host.frame_skipped();
            return;
        };
        if !self.render_thread_logged {
            self.render_thread_logged = true;
            // SAFETY: no input
            curia::info!("overlay frames run on the render thread", { thread: unsafe { GetCurrentThreadId() } });
        }
        if self.textures_lost_pending {
            self.textures_lost_pending = false;
            self.host.textures_lost();
        }
        // copies made on earlier Presents are read and reported before this frame, which hands them on
        self.capture.poll(&device.ctx, &mut HostCaptureSink::new(self.host));
        self.stream.poll(&device.ctx, &mut HostStreamSink::new(self.host));
        let input = Self::frame_input(shared, self.bound_window, self.ticks_per_second);
        let Some(frame) = self.host.frame(&input) else {
            self.host.frame_skipped();
            return;
        };
        if let Some(window) = self.bound_window {
            SizeMessage::post(
                HWND(window as *mut _),
                frame.size_message_width,
                frame.size_message_height,
                "host frame",
            );
        }
        if frame.release_stream {
            // a stream that ended drops its copies without telling the manager, which ended the stream itself
            self.stream.release();
        }
        let mut pass = DrawPass {
            device,
            renderer,
            capture: &mut self.capture,
            sink: HostCaptureSink::new(self.host),
            stream: &mut self.stream,
            stream_sink: HostStreamSink::new(self.host),
            size: self.targets.size(),
        };
        // SAFETY: OverlayHost promises the frame's pointers stay valid until its next frame, after this Present
        unsafe { Self::draw(&mut pass, frame, view, wrapped) };
    }

    // a frame with no layers, no capture of either set and no stream frame costs no D3D call
    unsafe fn draw(
        pass: &mut DrawPass<'_>,
        frame: &AmmHostFrame,
        view: &ID3D11RenderTargetView,
        wrapped: Option<&ID3D11Resource>,
    ) {
        // SAFETY: the caller vouches for the frame
        unsafe { pass.renderer.begin_frame(frame) };
        if frame.layer_count == 0
            && frame.capture_count == 0
            && frame.layer_capture_count == 0
            && frame.stream_count == 0
        {
            return;
        }
        match (pass.device.on12.clone(), wrapped) {
            // SAFETY: as above
            (Some(on12), Some(wrapped)) => unsafe { pass.wrapped(&on12, wrapped, view, frame) },
            // a D3D11On12 target always has its wrapper
            (Some(_), None) => {}
            // SAFETY: as above
            (None, _) => unsafe { pass.native(view, frame) },
        }
    }

    // the bound chain passes at once; a chain on another window is skipped; a new device on the bound window tears
    // everything down and builds it again
    fn accept_chain(&mut self, shared: &OverlayShared, chain: &IDXGISwapChain) -> bool {
        if self.targets.is_chain(chain) {
            return true;
        }
        // SAFETY: the chain is live for the Present that called this
        let Ok(desc) = (unsafe { chain.GetDesc() }) else {
            return false;
        };
        let window = desc.OutputWindow.0 as usize;
        if self.bound_window.is_some_and(|bound| bound != window) {
            return false;
        }
        let device_id = Self::chain_device(chain);
        if self.device.is_some() && device_id != self.bound_device {
            curia::info!("overlay sees a new device on the game window, rebuilding the overlay device");
            self.release_device(shared);
        }
        if !self.ensure_device(shared, chain, window, device_id) {
            return false;
        }
        // a new or resized chain drops the layer textures too, so they come again from the host
        self.release_targets();
        self.release_layer_textures();
        let Some(device) = self.device.as_ref() else {
            return false;
        };
        if self.targets.create(device, chain, &desc).is_err() {
            curia::warn!("overlay could not make the targets of the game's swap chain");
            self.release_targets();
            return false;
        }
        shared.record_size(desc.BufferDesc.Width, desc.BufferDesc.Height, desc.BufferDesc.Format);
        curia::info!("overlay bound", { width: desc.BufferDesc.Width, height: desc.BufferDesc.Height });
        self.ensure_renderer();
        true
    }

    fn ensure_device(
        &mut self,
        shared: &OverlayShared,
        chain: &IDXGISwapChain,
        window: usize,
        device_id: usize,
    ) -> bool {
        if self.device.is_some() {
            return true;
        }
        if self.device_failed {
            return false;
        }
        let device = match OverlayDevice::start(chain, &shared.queue) {
            DeviceStart::Ready(device) => device,
            DeviceStart::Waiting => {
                self.note_queue_wait();
                return false;
            }
            DeviceStart::Failed => {
                self.device_failed = true;
                return false;
            }
        };
        let path = if device.is_on12() { "D3D12 through D3D11On12" } else { "D3D11" };
        self.device = Some(device);
        self.bound_window = Some(window);
        self.bound_device = device_id;
        shared.set_game_window(window);
        if let Some(input) = self.input {
            WndProcRouter::install_bound(HWND(window as *mut _), input);
        }
        curia::info!("overlay device ready", { path: path });
        true
    }

    // the renderer's device objects are made once per device, at the first Present on it
    fn ensure_renderer(&mut self) {
        if self.renderer.is_some() || self.renderer_failed {
            return;
        }
        let Some(device) = self.device.as_ref() else {
            return;
        };
        match LayerRenderer::new(&device.device) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(error) => {
                self.renderer_failed = true;
                curia::error!("overlay layer renderer could not start, no layers are drawn on this device", {
                    error: error.to_string()
                });
            }
        }
    }

    fn note_queue_wait(&mut self) {
        if self.queue_wait_logged {
            return;
        }
        self.queue_wait_frames += 1;
        if self.queue_wait_frames < Self::QUEUE_WAIT_LOG_FRAMES {
            return;
        }
        self.queue_wait_logged = true;
        curia::warn!("overlay waits for the game's D3D12 command queue, none captured yet");
    }

    // every reference to a back buffer must be gone before ResizeBuffers; the D3D11On12 context is ours, so
    // clearing its state also unbinds a target left bound
    fn release_targets(&mut self) {
        self.targets.release();
        if let Some(device) = self.device.as_ref().filter(|device| device.is_on12()) {
            // SAFETY: the context is live and ours alone
            unsafe {
                device.ctx.ClearState();
                device.ctx.Flush();
            }
        }
    }

    // drops the layer textures, the game picture and the capture and stream staging textures; the host still counts
    // on them, so it is told before the next frame
    fn release_layer_textures(&mut self) {
        let capturing = self.capture.release();
        let streaming = self.stream.release();
        if let Some(renderer) = self.renderer.as_mut() {
            renderer.release_textures();
        }
        if self.renderer.is_some() || capturing || streaming {
            self.textures_lost_pending = true;
        }
    }

    fn release_device(&mut self, shared: &OverlayShared) {
        self.release_layer_textures();
        self.renderer = None;
        self.release_targets();
        self.device = None;
        shared.queue.clear();
        self.device_failed = false;
        self.renderer_failed = false;
        self.bound_device = 0;
        self.queue_wait_frames = 0;
        self.queue_wait_logged = false;
    }

    // the device pointer is identity only; the reference taken here is released on return
    fn chain_device(chain: &IDXGISwapChain) -> usize {
        // SAFETY: the chain is live for the Present that called this
        if let Ok(device) = unsafe { chain.GetDevice::<ID3D11Device>() } {
            return device.as_raw() as usize;
        }
        // SAFETY: as above
        unsafe { chain.GetDevice::<ID3D12Device>() }.map_or(0, |device| device.as_raw() as usize)
    }

    fn back_buffer_index(&self, chain: &IDXGISwapChain) -> Option<usize> {
        if !self.device.as_ref().is_some_and(OverlayDevice::is_on12) {
            return Some(0);
        }
        let chain = chain.cast::<IDXGISwapChain3>().ok()?;
        // SAFETY: the chain is live
        Some(unsafe { chain.GetCurrentBackBufferIndex() } as usize)
    }

    // layer coordinates are back-buffer pixels, the same pixels pointer events are sent in
    fn frame_input(shared: &OverlayShared, window: Option<usize>, ticks_per_second: f64) -> AmmHostFrameInput {
        let (width, height) = shared.back_buffer_size();
        // SAFETY: a window handle that is gone gives 0, not a fault
        let dpi = window.map_or(0, |window| unsafe { GetDpiForWindow(HWND(window as *mut _)) });
        let (client_width, client_height) = window.map_or((0, 0), Self::client_size);
        let mut counter = 0i64;
        // SAFETY: the out pointer is a writable local
        let _ = unsafe { QueryPerformanceCounter(&mut counter) };
        AmmHostFrameInput {
            width,
            height,
            pixels_per_point: if dpi != 0 { dpi as f32 / Self::DEFAULT_DPI } else { 1.0 },
            time_secs: counter as f64 / ticks_per_second,
            // SAFETY: no input
            now_ms: unsafe { GetTickCount64() },
            menu_screen: 0,
            format: shared.back_buffer_format().0 as u32,
            client_width,
            client_height,
        }
    }

    // the window's client size, 0 x 0 when the window is gone or minimised
    fn client_size(window: usize) -> (u32, u32) {
        let mut client = RECT::default();
        // SAFETY: the out pointer is a writable local; a window that is gone fails the call
        if unsafe { GetClientRect(HWND(window as *mut _), &mut client) }.is_err() {
            return (0, 0);
        }
        if client.right > 0 && client.bottom > 0 { (client.right as u32, client.bottom as u32) } else { (0, 0) }
    }
}
