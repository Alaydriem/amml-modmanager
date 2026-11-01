use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Barrier, Mutex};

use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};
use amm_overlay::OverlayHost;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT;

use super::swap_window::SwapWindow;
use super::{TestFrame, TestMesh};

/// A finished capture: its sequence, whether it worked, and its pixels with their row pitch.
pub struct CaptureResult {
    pub sequence: u32,
    pub ok: bool,
    pub pixels: Vec<u8>,
    pub row_pitch: u32,
}

impl CaptureResult {
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let at = (y * self.row_pitch + x * 4) as usize;
        [self.pixels[at], self.pixels[at + 1], self.pixels[at + 2], self.pixels[at + 3]]
    }
}

/// Two barriers a frame waits on: the test learns the render thread is inside `frame`, then lets it go.
pub struct FrameGate {
    pub entered: Barrier,
    pub release: Barrier,
}

/// An `OverlayHost` whose every frame is one full-screen red layer of a mod. It counts what the overlay asks of it.
pub struct TestHost {
    pub ready: AtomicBool,
    pub frames: AtomicU32,
    pub textures_lost: AtomicU32,
    pub skipped: AtomicU32,
    pub panic_in_frame: AtomicBool,
    /// The next frame also asks for layer capture `CAPTURE_SEQUENCE`.
    pub want_capture: AtomicBool,
    /// The next frame offers stream frame `STREAM_ID`.
    pub want_stream: AtomicBool,
    /// The next frame offers stream frame `STREAM_ID` and game-only capture `GAME_CAPTURE_SEQUENCE`.
    pub want_stream_and_capture: AtomicBool,
    /// The next frame sets `release_stream`; the stream reports after it are recorded as released.
    pub want_release_stream: AtomicBool,
    /// The next frame carries the size message `SIZE_MESSAGE`.
    pub want_size_message: AtomicBool,
    /// The forced size `render_size` gives; `None` keeps the asked size.
    pub forced_size: Mutex<Option<(u32, u32)>>,
    render_size_asks: AtomicU32,
    released: AtomicBool,
    stream_done: Mutex<Vec<(u32, bool, bool)>>,
    stream_buffer: Mutex<Vec<u8>>,
    pub last_size: Mutex<(u32, u32)>,
    pub gate: Mutex<Option<Arc<FrameGate>>>,
    pub capture: Mutex<Option<CaptureResult>>,
    buffer: Mutex<Vec<u8>>,
    pitch: AtomicU32,
    plain: AmmHostFrame,
    with_capture: AmmHostFrame,
    with_stream: AmmHostFrame,
    with_stream_and_capture: AmmHostFrame,
    with_release: AmmHostFrame,
    with_size_message: AmmHostFrame,
    _frame: TestFrame,
    _mesh: TestMesh,
}

// SAFETY: the frames and the mesh they point into are never changed after `new`; the rest are atomics and locks
unsafe impl Sync for TestHost {}

// SAFETY: the frames point into `_mesh` and `_frame`, which live as long as the host; the capture buffer is sized
// for every row before its pointer is handed out and is not touched again until `capture_done`
unsafe impl OverlayHost for TestHost {
    fn game_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    fn frame(&self, input: &AmmHostFrameInput) -> Option<&AmmHostFrame> {
        if self.panic_in_frame.load(Ordering::SeqCst) {
            panic!("test host frame panics");
        }
        let gate = self.gate.lock().unwrap().take();
        if let Some(gate) = gate {
            gate.entered.wait();
            gate.release.wait();
        }
        self.frames.fetch_add(1, Ordering::SeqCst);
        *self.last_size.lock().unwrap() = (input.width, input.height);
        if self.want_size_message.swap(false, Ordering::SeqCst) {
            return Some(&self.with_size_message);
        }
        if self.want_release_stream.swap(false, Ordering::SeqCst) {
            self.released.store(true, Ordering::SeqCst);
            return Some(&self.with_release);
        }
        if self.want_stream_and_capture.swap(false, Ordering::SeqCst) {
            return Some(&self.with_stream_and_capture);
        }
        if self.want_stream.swap(false, Ordering::SeqCst) {
            return Some(&self.with_stream);
        }
        if self.want_capture.swap(false, Ordering::SeqCst) { Some(&self.with_capture) } else { Some(&self.plain) }
    }

    fn textures_lost(&self) {
        self.textures_lost.fetch_add(1, Ordering::SeqCst);
    }

    fn capture_buffer(&self, _sequence: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)> {
        let pitch = width * 4;
        let mut buffer = self.buffer.lock().unwrap();
        buffer.clear();
        buffer.resize((pitch * height) as usize, 0);
        self.pitch.store(pitch, Ordering::SeqCst);
        NonNull::new(buffer.as_mut_ptr()).map(|pointer| (pointer, pitch))
    }

    fn capture_done(&self, sequence: u32, ok: bool, _format: DXGI_FORMAT) {
        let pixels = self.buffer.lock().unwrap().clone();
        let row_pitch = self.pitch.load(Ordering::SeqCst);
        *self.capture.lock().unwrap() = Some(CaptureResult { sequence, ok, pixels, row_pitch });
    }

    fn stream_buffer(&self, _id: u32, width: u32, height: u32) -> Option<(NonNull<u8>, u32)> {
        let pitch = width * 4;
        let mut buffer = self.stream_buffer.lock().unwrap();
        buffer.clear();
        buffer.resize((pitch * height) as usize, 0);
        NonNull::new(buffer.as_mut_ptr()).map(|pointer| (pointer, pitch))
    }

    fn stream_done(&self, id: u32, ok: bool, _format: DXGI_FORMAT) {
        self.stream_done.lock().unwrap().push((id, ok, self.released.load(Ordering::SeqCst)));
    }

    fn render_size(&self, _width: u32, _height: u32) -> Option<(u32, u32)> {
        self.render_size_asks.fetch_add(1, Ordering::SeqCst);
        *self.forced_size.lock().unwrap()
    }

    fn size_forced(&self) -> bool {
        self.forced_size.lock().unwrap().is_some()
    }

    fn frame_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::SeqCst);
    }
}

impl TestHost {
    pub const CAPTURE_SEQUENCE: u32 = 7;
    pub const STREAM_ID: u32 = 7;
    pub const GAME_CAPTURE_SEQUENCE: u32 = 3;
    // more than FrameCapture::MAP_WAIT_FRAMES, so a stream copy is either read or failed by then
    pub const STREAM_PRESENTS: u32 = 40;
    pub const SIZE_MESSAGE: (u32, u32) = (800, 450);
    pub const RED: [u8; 4] = [255, 0, 0, 255];
    // more than FrameCapture::MAP_WAIT_FRAMES, so a capture is either read or failed by then
    const CAPTURE_PRESENTS: u32 = 40;
    const WHITE_TEXTURE: u64 = 1;
    const MOD_OWNER: u8 = 0;
    const EVERYWHERE: [f32; 4] = [0.0, 0.0, 4096.0, 4096.0];

    /// A host that lives for the rest of the test process, as `Overlay::install` needs.
    pub fn leaked() -> &'static TestHost {
        Box::leak(Box::new(Self::new()))
    }

    fn new() -> Self {
        let mut mesh = TestMesh::default();
        mesh.full(Self::WHITE_TEXTURE, 1, 1, [255, 255, 255, 255]).quad(
            Self::EVERYWHERE,
            Self::RED,
            Self::WHITE_TEXTURE,
            Self::EVERYWHERE,
        );
        let frame = TestFrame::new(&[(Self::MOD_OWNER, &mesh)]);
        let plain = frame.frame();
        let mut with_capture = plain;
        with_capture.layer_capture_count = 1;
        with_capture.layer_captures[0] = Self::CAPTURE_SEQUENCE;
        let mut with_stream = plain;
        with_stream.stream_count = 1;
        with_stream.streams[0] = Self::STREAM_ID;
        let mut with_stream_and_capture = with_stream;
        with_stream_and_capture.capture_count = 1;
        with_stream_and_capture.captures[0] = Self::GAME_CAPTURE_SEQUENCE;
        let mut with_release = plain;
        with_release.release_stream = true;
        let mut with_size_message = plain;
        with_size_message.size_message_width = Self::SIZE_MESSAGE.0;
        with_size_message.size_message_height = Self::SIZE_MESSAGE.1;
        Self {
            ready: AtomicBool::new(false),
            frames: AtomicU32::new(0),
            textures_lost: AtomicU32::new(0),
            skipped: AtomicU32::new(0),
            panic_in_frame: AtomicBool::new(false),
            want_capture: AtomicBool::new(false),
            want_stream: AtomicBool::new(false),
            want_stream_and_capture: AtomicBool::new(false),
            want_release_stream: AtomicBool::new(false),
            want_size_message: AtomicBool::new(false),
            forced_size: Mutex::new(None),
            render_size_asks: AtomicU32::new(0),
            released: AtomicBool::new(false),
            stream_done: Mutex::new(Vec::new()),
            stream_buffer: Mutex::new(Vec::new()),
            last_size: Mutex::new((0, 0)),
            gate: Mutex::new(None),
            capture: Mutex::new(None),
            buffer: Mutex::new(Vec::new()),
            pitch: AtomicU32::new(0),
            plain,
            with_capture,
            with_stream,
            with_stream_and_capture,
            with_release,
            with_size_message,
            _frame: frame,
            _mesh: mesh,
        }
    }

    pub fn set_ready(&self) {
        self.ready.store(true, Ordering::SeqCst);
    }

    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::SeqCst)
    }

    pub fn textures_lost(&self) -> u32 {
        self.textures_lost.load(Ordering::SeqCst)
    }

    /// How many times the overlay asked for a forced render size.
    pub fn render_size_asks(&self) -> u32 {
        self.render_size_asks.load(Ordering::SeqCst)
    }

    pub fn last_size(&self) -> (u32, u32) {
        *self.last_size.lock().unwrap()
    }

    /// The stream frame reports so far, as (id, ok, came after a release).
    pub fn stream_reports(&self) -> Vec<(u32, bool, bool)> {
        self.stream_done.lock().unwrap().clone()
    }

    pub fn take_capture(&self) -> Option<CaptureResult> {
        self.capture.lock().unwrap().take()
    }

    /// Asks the next frame for a layer capture and presents `window` until the capture is reported.
    pub fn capture_of(&self, window: &SwapWindow) -> CaptureResult {
        self.capture_with(|| window.present())
    }

    /// As `capture_of`, presenting through `present`.
    pub fn capture_with(&self, present: impl Fn()) -> CaptureResult {
        self.want_capture.store(true, Ordering::SeqCst);
        for _ in 0..Self::CAPTURE_PRESENTS {
            present();
            if let Some(capture) = self.take_capture() {
                return capture;
            }
        }
        panic!("no capture reported after {} presents", Self::CAPTURE_PRESENTS);
    }

    /// Makes the next frame wait at the returned gate.
    pub fn arm_gate(&self) -> Arc<FrameGate> {
        let gate = Arc::new(FrameGate { entered: Barrier::new(2), release: Barrier::new(2) });
        *self.gate.lock().unwrap() = Some(gate.clone());
        gate
    }
}
