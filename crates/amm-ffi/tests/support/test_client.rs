use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};

use amm_abi::types::{
    AmmAcquireResult, AmmApi, AmmCallbacks, AmmCapability, AmmEvent, AmmEventKind, AmmFrameInput, AmmFrameOutput,
    AmmHandle, AmmStatus, AmmUiEventKind,
};

use super::{TestHost, TestMesh};

/// A reply for one layer: `(wants_pointer, wants_keyboard, cursor, layer_rect)`.
pub type LayerReply = (bool, bool, u8, [f32; 4]);

/// One frame call as the client saw it: its `allowed_layers` and the kind and down flag of each event.
pub type SeenCall = (u8, Vec<SeenEvent>);

/// The kind and down flag of one event a frame call got.
pub type SeenEvent = (AmmUiEventKind, bool);

/// What a test client does in its callbacks, and what it saw.
pub struct ClientState {
    pub mesh: TestMesh,
    pub status: AtomicI32,
    pub cursor: AtomicU8,
    pub frames: AtomicU32,
    pub textures_lost: AtomicU32,
    pub allowed: AtomicU8,
    pub input_flags: AtomicU8,
    pub events_seen: AtomicU32,
    /// When set, each frame calls `acquire` on this handle and keeps the outcome byte in `inner_outcome`.
    pub call_back_in: Mutex<Option<AmmHandle>>,
    pub inner_outcome: AtomicU8,
    pub called_back: AtomicBool,
    /// When set, each frame calls `unregister` on this handle and keeps the status in `inner_unregister`.
    pub unregister_in: Mutex<Option<AmmHandle>>,
    pub inner_unregister: AtomicI32,
    /// When set, each frame sends on the first channel, then waits on the second.
    pub gate: Mutex<Option<(Sender<()>, Receiver<()>)>>,
    /// Each frame call, in call order.
    pub calls: Mutex<Vec<SeenCall>>,
    /// When set, the frame call for the Hud layer (index 0) or the Overlay layer (index 1) replies with this
    /// reply in place of the default reply.
    pub layer_replies: Mutex<[Option<LayerReply>; 2]>,
    /// The `allowed_layers` bits whose frame calls return `InvalidArgument` in place of `status`.
    pub fail_layers: AtomicU8,
}

/// One mod registered through the table with raw callbacks.
pub struct TestClient {
    state: Arc<ClientState>,
    handle: Option<AmmHandle>,
    config_dir: PathBuf,
}

impl TestClient {
    pub fn register(host: &TestHost, descriptor: &str, mesh: TestMesh) -> TestClient {
        Self::try_register(host, descriptor, mesh).unwrap_or_else(|status| panic!("register failed: {status:?}"))
    }

    pub fn try_register(host: &TestHost, descriptor: &str, mesh: TestMesh) -> Result<TestClient, AmmStatus> {
        let config_dir = host.next_mod_dir();
        let dir_text = config_dir.display().to_string();
        let (state, callbacks) = Self::detached(mesh);
        let mut handle = AmmHandle { slot: u16::MAX, generation: u16::MAX };
        let register = Self::api().register.unwrap_or_else(|| panic!("register"));
        // SAFETY: the descriptor, config folder, callbacks and handle live for the call
        let status = unsafe {
            register(descriptor.as_ptr(), descriptor.len(), dir_text.as_ptr(), dir_text.len(), &callbacks, &mut handle)
        };
        match status {
            AmmStatus::Ok => Ok(Self { state, handle: Some(handle), config_dir }),
            other => Err(other),
        }
    }

    /// Client state and callbacks not registered anywhere, for a test that drives a `HostRuntime` directly.
    /// The state must outlive every call of the callbacks.
    pub fn detached(mesh: TestMesh) -> (Arc<ClientState>, AmmCallbacks) {
        let state = Arc::new(ClientState {
            mesh,
            status: AtomicI32::new(AmmStatus::Ok as i32),
            cursor: AtomicU8::new(0),
            frames: AtomicU32::new(0),
            textures_lost: AtomicU32::new(0),
            allowed: AtomicU8::new(0),
            input_flags: AtomicU8::new(0),
            events_seen: AtomicU32::new(0),
            call_back_in: Mutex::new(None),
            inner_outcome: AtomicU8::new(u8::MAX),
            called_back: AtomicBool::new(false),
            unregister_in: Mutex::new(None),
            inner_unregister: AtomicI32::new(i32::MIN),
            gate: Mutex::new(None),
            calls: Mutex::new(Vec::new()),
            layer_replies: Mutex::new([None; 2]),
            fail_layers: AtomicU8::new(0),
        });
        let callbacks = AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: Arc::as_ptr(&state).cast_mut().cast::<c_void>(),
            frame: Some(Self::on_frame),
            textures_lost: Some(Self::on_textures_lost),
        };
        (state, callbacks)
    }

    pub fn api() -> &'static AmmApi {
        TestHost::api()
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn handle(&self) -> AmmHandle {
        self.handle.unwrap_or_else(|| panic!("registered"))
    }

    pub fn owner(&self) -> u8 {
        self.handle().slot as u8
    }

    pub fn state(&self) -> &Arc<ClientState> {
        &self.state
    }

    pub fn frames(&self) -> u32 {
        self.state.frames.load(Ordering::SeqCst)
    }

    pub fn textures_lost(&self) -> u32 {
        self.state.textures_lost.load(Ordering::SeqCst)
    }

    pub fn fail_with(&self, status: AmmStatus) {
        self.state.status.store(status as i32, Ordering::SeqCst);
    }

    pub fn set_cursor_byte(&self, cursor: u8) {
        self.state.cursor.store(cursor, Ordering::SeqCst);
    }

    /// The `input_flags` of the last frame call.
    pub fn input_flags(&self) -> u8 {
        self.state.input_flags.load(Ordering::SeqCst)
    }

    /// The `allowed_layers` of each frame call so far.
    pub fn calls(&self) -> Vec<u8> {
        self.seen_calls().into_iter().map(|(allowed, _)| allowed).collect()
    }

    /// Each frame call so far with the events it got.
    pub fn seen_calls(&self) -> Vec<SeenCall> {
        self.state.calls.lock().unwrap_or_else(|error| panic!("calls: {error}")).clone()
    }

    /// Makes the frame call of one layer (0 Hud, 1 Overlay) reply with this reply.
    pub fn reply_for_layer(&self, layer: usize, reply: LayerReply) {
        self.state.layer_replies.lock().unwrap_or_else(|error| panic!("replies: {error}"))[layer] = Some(reply);
    }

    pub fn acquire(&self, capability: AmmCapability) -> AmmAcquireResult {
        self.acquire_raw(capability as u8)
    }

    pub fn acquire_raw(&self, capability: u8) -> AmmAcquireResult {
        // SAFETY: acquire takes plain values
        unsafe { Self::api().acquire.unwrap_or_else(|| panic!("acquire"))(self.handle(), capability) }
    }

    pub fn release_raw(&self, capability: u8) {
        // SAFETY: release takes plain values
        unsafe { Self::api().release.unwrap_or_else(|| panic!("release"))(self.handle(), capability) }
    }

    pub fn set_cursor_mode(&self, on: u8) {
        // SAFETY: set_cursor_mode takes plain values
        unsafe { Self::api().set_cursor_mode.unwrap_or_else(|| panic!("set_cursor_mode"))(self.handle(), on) }
    }

    pub fn poll(&self) -> Vec<AmmEvent> {
        Self::poll_handle(self.handle())
    }

    pub fn poll_handle(handle: AmmHandle) -> Vec<AmmEvent> {
        let blank =
            AmmEvent { kind: AmmEventKind::Enabled, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 };
        let mut buffer = vec![blank; 64];
        // SAFETY: the buffer has room for its length in events
        let count = unsafe {
            Self::api().poll_events.unwrap_or_else(|| panic!("poll_events"))(handle, buffer.as_mut_ptr(), buffer.len())
        };
        buffer.truncate(count);
        buffer
    }

    pub fn log(&self, level: u8, message: &str, fields_json: &str) {
        Self::log_handle(self.handle(), level, message, fields_json);
    }

    pub fn log_handle(handle: AmmHandle, level: u8, message: &str, fields_json: &str) {
        let (target, file) = ("test", "test.rs");
        // SAFETY: every text lives for the call
        unsafe {
            Self::api().log.unwrap_or_else(|| panic!("log"))(
                handle,
                level,
                target.as_ptr(),
                target.len(),
                message.as_ptr(),
                message.len(),
                fields_json.as_ptr(),
                fields_json.len(),
                file.as_ptr(),
                file.len(),
                7,
            )
        }
    }

    pub fn unregister(&mut self) {
        if let Some(handle) = self.handle.take() {
            Self::unregister_handle(handle);
        }
    }

    pub fn unregister_handle(handle: AmmHandle) -> AmmStatus {
        // SAFETY: unregister takes a plain handle
        unsafe { Self::api().unregister.unwrap_or_else(|| panic!("unregister"))(handle) }
    }

    /// The id of the holder of `capability` through the table, copied into a buffer of `capacity` bytes, with the
    /// length the table returned.
    pub fn holder_id(capability: u8, capacity: usize) -> (u32, Vec<u8>) {
        let mut buffer = vec![0u8; capacity];
        // SAFETY: the buffer has room for its length in bytes for the call
        let len = unsafe {
            Self::api().holder.unwrap_or_else(|| panic!("holder"))(capability, buffer.as_mut_ptr(), buffer.len())
        };
        (len, buffer)
    }

    unsafe extern "C" fn on_frame(user: *mut c_void, input: *const AmmFrameInput, out: *mut AmmFrameOutput) -> i32 {
        // SAFETY: user is the ClientState of a live TestClient, and input and out are valid for the call
        let (state, input, out) = unsafe { (&*user.cast::<ClientState>(), &*input, &mut *out) };
        if let Some((entered, release)) = state.gate.lock().unwrap_or_else(|error| panic!("gate: {error}")).as_ref() {
            let _ = entered.send(());
            let _ = release.recv();
        }
        let call_back = *state.call_back_in.lock().unwrap_or_else(|error| panic!("call back: {error}"));
        if let Some(handle) = call_back {
            let acquire = Self::api().acquire.unwrap_or_else(|| panic!("acquire"));
            // SAFETY: acquire takes plain values
            let result = unsafe { acquire(handle, AmmCapability::Camera as u8) };
            state.inner_outcome.store(result.outcome as u8, Ordering::SeqCst);
            state.called_back.store(true, Ordering::SeqCst);
        }
        let unregister_in = *state.unregister_in.lock().unwrap_or_else(|error| panic!("unregister in: {error}"));
        if let Some(handle) = unregister_in {
            state.inner_unregister.store(Self::unregister_handle(handle) as i32, Ordering::SeqCst);
        }
        state.frames.fetch_add(1, Ordering::SeqCst);
        state.allowed.store(input.allowed_layers, Ordering::SeqCst);
        state.input_flags.store(input.input_flags, Ordering::SeqCst);
        state.events_seen.fetch_add(input.event_count as u32, Ordering::SeqCst);
        out.mesh = state.mesh.view();
        out.wants_pointer = 0;
        out.wants_keyboard = 0;
        out.cursor = state.cursor.load(Ordering::SeqCst);
        out.layer_rect = [0.0, 0.0, 100.0, 100.0];
        let events = if input.event_count == 0 {
            Vec::new()
        } else {
            // SAFETY: the host keeps event_count events at events valid for the call
            unsafe { std::slice::from_raw_parts(input.events, input.event_count) }
                .iter()
                .map(|event| (event.kind, event.down))
                .collect()
        };
        state.calls.lock().unwrap_or_else(|error| panic!("calls: {error}")).push((input.allowed_layers, events));
        let layer = usize::from(input.allowed_layers == 2);
        if let Some((wants_pointer, wants_keyboard, cursor, rect)) =
            state.layer_replies.lock().unwrap_or_else(|error| panic!("replies: {error}"))[layer]
        {
            out.wants_pointer = u8::from(wants_pointer);
            out.wants_keyboard = u8::from(wants_keyboard);
            out.cursor = cursor;
            out.layer_rect = rect;
        }
        if input.allowed_layers & state.fail_layers.load(Ordering::SeqCst) != 0 {
            return AmmStatus::InvalidArgument as i32;
        }
        state.status.load(Ordering::SeqCst)
    }

    unsafe extern "C" fn on_textures_lost(user: *mut c_void) {
        // SAFETY: user is the ClientState of a live TestClient
        let state = unsafe { &*user.cast::<ClientState>() };
        state.textures_lost.fetch_add(1, Ordering::SeqCst);
    }
}

impl Drop for TestClient {
    fn drop(&mut self) {
        self.unregister();
    }
}
