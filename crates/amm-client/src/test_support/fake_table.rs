use std::cell::RefCell;
use std::path::Path;

use amm_abi::types::{
    AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmAcquireOutcome, AmmAcquireResult, AmmApi, AmmCallbacks, AmmCapability,
    AmmCaptureStreamFrame, AmmCaptureStreamRequest, AmmEvent, AmmFrameInput, AmmFrameOutput, AmmHandle, AmmRenderSize,
    AmmRenderSizeState, AmmStatus,
};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::traits::AbiSized;

#[cfg(feature = "egui")]
use crate::link::FrameHandler;
use crate::link::{ApiTable, ManagerLink};

use super::{FakeState, LoggedCall, SettingCall};

thread_local! {
    static STATE: RefCell<FakeState> = RefCell::new(FakeState::new());
}

/// A manager function table for a client's tests. Each function records its call in a `FakeState` of the
/// calling thread and answers from it, so each test thread sees its own fake manager.
pub struct FakeTable;

impl FakeTable {
    pub const CONFIG_DIR: &'static str = "C:\\mods\\alaydriem-test\\config";
    pub const DESCRIPTOR: &'static str = "namespace = \"test\"\nname = \"alaydriem-test\"\n";

    /// A full table of the current ABI with every slot filled.
    pub fn api() -> AmmApi {
        AmmApi {
            size: size_of::<AmmApi>() as u32,
            abi_major: AMM_ABI_MAJOR,
            abi_minor: AMM_ABI_MINOR,
            register: Some(Self::register),
            unregister: Some(Self::unregister),
            config_document: Some(Self::config_document),
            acquire: Some(Self::acquire),
            release: Some(Self::release),
            holder: Some(Self::holder),
            poll_events: Some(Self::poll_events),
            key_down: Some(Self::key_down),
            set_setting: Some(Self::set_setting),
            set_cursor_mode: Some(Self::set_cursor_mode),
            log: Some(Self::log),
            request_screenshot: Some(Self::request_screenshot),
            held_by_other: Some(Self::held_by_other),
            texture_budget: Some(Self::texture_budget),
            request_screenshot_layers: Some(Self::request_screenshot_layers),
            register_menu_entry: Some(Self::register_menu_entry),
            locale: Some(Self::locale),
            set_icon: Some(Self::set_icon),
            game_picture: Some(Self::game_picture),
            set_game_viewport: Some(Self::set_game_viewport),
            set_render_size: Some(Self::set_render_size),
            render_size: Some(Self::render_size),
            capture_stream_request: Some(Self::capture_stream_request),
            capture_stream_take: Some(Self::capture_stream_take),
            capture_stream_free: Some(Self::capture_stream_free),
            capture_stream_close: Some(Self::capture_stream_close),
            present_count: Some(Self::present_count),
            capture_stream_lend: Some(Self::capture_stream_lend),
            capture_stream_give_back: Some(Self::capture_stream_give_back),
            watch_keys: Some(Self::watch_keys),
            send_key: Some(Self::send_key),
        }
    }

    /// A table of ABI minor 0: it ends before `watch_keys`, and both slots are null.
    pub fn api_minor_0() -> AmmApi {
        AmmApi { size: AmmApi::BASE_SIZE as u32, abi_minor: 0, watch_keys: None, send_key: None, ..Self::api() }
    }

    /// `api` leaked, so it lives for the rest of the process as the manager's table does.
    pub fn leak(api: AmmApi) -> &'static AmmApi {
        Box::leak(Box::new(api))
    }

    /// A checked table over a leaked `api()`.
    pub fn table() -> ApiTable {
        let api = Self::leak(Self::api());
        // SAFETY: the table is leaked, so it stays valid for the process, and it is a complete AmmApi
        match unsafe { ApiTable::new(api) } {
            Ok(table) => table,
            Err(error) => panic!("fake table rejected: {error}"),
        }
    }

    /// A link registered on a fresh fake table with `DESCRIPTOR`, with `handler` as its egui pass.
    #[cfg(feature = "egui")]
    pub fn link<H: FrameHandler + Send + 'static>(handler: H) -> ManagerLink {
        match ManagerLink::new(Self::table(), Self::DESCRIPTOR, Path::new(Self::CONFIG_DIR), handler) {
            Ok(link) => link,
            Err(error) => panic!("register failed: {error}"),
        }
    }

    /// A link with no frame callback on a leaked copy of `api`, for a test of a table with some slots missing.
    pub fn link_without_frame_on(api: AmmApi) -> ManagerLink {
        // SAFETY: the table is leaked, so it stays valid for the process, and it is a complete AmmApi
        let table = match unsafe { ApiTable::new(Self::leak(api)) } {
            Ok(table) => table,
            Err(error) => panic!("fake table rejected: {error}"),
        };
        match ManagerLink::without_frame(table, Self::DESCRIPTOR, Path::new(Self::CONFIG_DIR)) {
            Ok(link) => link,
            Err(error) => panic!("register failed: {error}"),
        }
    }

    /// A link registered on a fresh fake table with `DESCRIPTOR` and no frame callback.
    pub fn link_without_frame() -> ManagerLink {
        match ManagerLink::without_frame(Self::table(), Self::DESCRIPTOR, Path::new(Self::CONFIG_DIR)) {
            Ok(link) => link,
            Err(error) => panic!("register failed: {error}"),
        }
    }

    /// Runs `f` over this thread's fake state.
    pub fn with<R>(f: impl FnOnce(&mut FakeState) -> R) -> R {
        STATE.with(|state| f(&mut state.borrow_mut()))
    }

    /// Queues `event` for the next `poll_events`.
    pub fn push_event(event: ModEvent) {
        Self::with(|state| state.events.push_back(AmmEvent::from(event)));
    }

    /// Sets the text `config_document` returns.
    pub fn set_config(text: &str) {
        Self::with(|state| state.config = text.to_owned());
    }

    /// Sets what `acquire` answers for `capability`. A capability with no outcome set is granted.
    pub fn set_acquire_outcome(capability: Capability, outcome: AcquireOutcome) {
        Self::with(|state| state.acquire_outcomes.insert(capability, outcome));
    }

    /// Sets what `request_screenshot` answers: the status, and on `Ok` the sequence.
    pub fn set_screenshot_answer(status: AmmStatus, sequence: u32) {
        Self::with(|state| {
            state.screenshot_status = status;
            state.screenshot_sequence = sequence;
        });
    }

    /// Sets what `held_by_other` answers for `capability`. A capability with no answer set is not held by another
    /// mod.
    pub fn set_held_by_other(capability: Capability, held: bool) {
        Self::with(|state| state.held_by_other.insert(capability, held));
    }

    /// Sets the id `register_menu_entry` answers with.
    pub fn set_menu_entry_id(id: u16) {
        Self::with(|state| state.menu_entry_id = id);
    }

    /// Sets the grant `texture_budget` answers with.
    pub fn set_texture_grant(grant: TextureGrant) {
        Self::with(|state| state.texture_grant = grant);
    }

    /// Runs the registered frame callback once over an empty input.
    pub fn run_frame() -> i32 {
        let Some(callbacks) = Self::with(|state| state.callbacks) else { panic!("nothing registered") };
        let Some(frame) = callbacks.frame else { panic!("no frame callback") };
        let input = AmmFrameInput {
            width: 640,
            height: 480,
            pixels_per_point: 1.0,
            allowed_layers: 3,
            input_flags: 7,
            ..AmmFrameInput::blank()
        };
        let mut out = Self::empty_output();
        // SAFETY: the user pointer is the one the link registered, and input and out live for the call
        unsafe { frame(callbacks.user, &input, &mut out) }
    }

    /// A frame output with every pointer null and every count zero.
    pub fn empty_output() -> AmmFrameOutput {
        AmmFrameOutput::blank()
    }

    unsafe fn text(ptr: *const u8, len: usize) -> String {
        if ptr.is_null() || len == 0 {
            return String::new();
        }
        // SAFETY: the client passes a pointer to len bytes that live for the call
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    fn capability(code: u8) -> Option<Capability> {
        AmmCapability::try_from(code).ok().map(Capability::from)
    }

    unsafe extern "C" fn register(
        descriptor_hcl: *const u8,
        descriptor_len: usize,
        config_dir: *const u8,
        config_dir_len: usize,
        callbacks: *const AmmCallbacks,
        out_handle: *mut AmmHandle,
    ) -> AmmStatus {
        // SAFETY: the client passes its descriptor text for the call
        let descriptor = unsafe { Self::text(descriptor_hcl, descriptor_len) };
        // SAFETY: the client passes its config folder text for the call
        let config_dir = unsafe { Self::text(config_dir, config_dir_len) };
        Self::with(|state| {
            state.descriptor = Some(descriptor);
            state.config_dir = Some(config_dir);
            if state.register_status != AmmStatus::Ok {
                return state.register_status;
            }
            // SAFETY: the client passes live callbacks and a writable handle for the call
            unsafe {
                state.callbacks = Some(*callbacks);
                *out_handle = state.handle;
            }
            AmmStatus::Ok
        })
    }

    unsafe extern "C" fn unregister(handle: AmmHandle) -> AmmStatus {
        Self::with(|state| {
            state.unregistered.push(handle);
            state.unregister_status
        })
    }

    unsafe extern "C" fn config_document(_: AmmHandle, out_ptr: *mut *const u8, out_len: *mut usize) -> AmmStatus {
        // The text stays in this thread's state, unchanged until the test sets it again
        let (ptr, len) = Self::with(|state| (state.config.as_ptr(), state.config.len()));
        // SAFETY: the client passes writable out pointers for the call
        unsafe {
            *out_ptr = ptr;
            *out_len = len;
        }
        AmmStatus::Ok
    }

    unsafe extern "C" fn acquire(_: AmmHandle, capability: u8) -> AmmAcquireResult {
        let Some(capability) = Self::capability(capability) else {
            return AmmAcquireResult { outcome: AmmAcquireOutcome::Denied, holder: AmmAcquireResult::NO_HOLDER };
        };
        let outcome = Self::with(|state| {
            state.acquired.push(capability);
            state.acquire_outcomes.get(&capability).copied().unwrap_or(AcquireOutcome::Granted)
        });
        AmmAcquireResult::from(outcome)
    }

    unsafe extern "C" fn release(_: AmmHandle, capability: u8) {
        if let Some(capability) = Self::capability(capability) {
            Self::with(|state| state.released.push(capability));
        }
    }

    unsafe extern "C" fn holder(_: u8, out_buf: *mut u8, out_cap: usize) -> u32 {
        let Some(id) = Self::with(|state| state.holder_id) else {
            return 0;
        };
        if id.len() <= out_cap {
            // SAFETY: the client passes a buffer with room for out_cap bytes
            unsafe { std::ptr::copy_nonoverlapping(id.as_ptr(), out_buf, id.len()) };
        }
        id.len() as u32
    }

    unsafe extern "C" fn locale(_: AmmHandle, out_buf: *mut u8, out_cap: usize) -> u32 {
        let code = Self::with(|state| {
            state.locale_reads += 1;
            state.locale
        });
        if !code.is_empty() && code.len() <= out_cap {
            // SAFETY: the client passes a buffer with room for out_cap bytes
            unsafe { std::ptr::copy_nonoverlapping(code.as_ptr(), out_buf, code.len()) };
        }
        code.len() as u32
    }

    unsafe extern "C" fn poll_events(_: AmmHandle, out_buf: *mut AmmEvent, capacity: usize) -> usize {
        Self::with(|state| {
            let mut written = 0;
            while written < capacity {
                let Some(event) = state.events.pop_front() else { break };
                // SAFETY: the client passes a buffer with room for capacity events
                unsafe { out_buf.add(written).write(event) };
                written += 1;
            }
            written
        })
    }

    unsafe extern "C" fn key_down(_: AmmHandle, _: u32) -> bool {
        false
    }

    unsafe extern "C" fn set_setting(
        _: AmmHandle,
        path: *const u8,
        path_len: usize,
        expression: *const u8,
        expression_len: usize,
    ) -> AmmStatus {
        // SAFETY: the client passes the path and expression texts for the call
        let call = unsafe {
            SettingCall { path: Self::text(path, path_len), expression: Self::text(expression, expression_len) }
        };
        Self::with(|state| {
            state.settings.push(call);
            state.set_setting_status
        })
    }

    unsafe extern "C" fn set_cursor_mode(_: AmmHandle, _: u8) {}

    unsafe extern "C" fn request_screenshot(
        _: AmmHandle,
        path: *const u8,
        path_len: usize,
        out_sequence: *mut u32,
    ) -> AmmStatus {
        // SAFETY: the client passes the path text for the call
        let path = unsafe { Self::text(path, path_len) };
        let (status, sequence) = Self::with(|state| {
            state.screenshot_requests.push(path);
            (state.screenshot_status, state.screenshot_sequence)
        });
        if status == AmmStatus::Ok {
            // SAFETY: the client passes a writable out sequence for the call
            unsafe { out_sequence.write(sequence) };
        }
        status
    }

    unsafe extern "C" fn request_screenshot_layers(
        handle: AmmHandle,
        path: *const u8,
        path_len: usize,
        layers: u8,
        out_sequence: *mut u32,
    ) -> AmmStatus {
        Self::with(|state| state.screenshot_layers.push(layers));
        // SAFETY: the client passes the path text and a writable out sequence for the call
        unsafe { Self::request_screenshot(handle, path, path_len, out_sequence) }
    }

    unsafe extern "C" fn held_by_other(_: AmmHandle, capability: u8) -> u8 {
        let Some(capability) = Self::capability(capability) else {
            return 0;
        };
        u8::from(Self::with(|state| state.held_by_other.get(&capability).copied().unwrap_or(false)))
    }

    unsafe extern "C" fn register_menu_entry(
        _: AmmHandle,
        label: *const u8,
        label_len: usize,
        mark: *const u8,
        mark_len: usize,
        screens: u8,
        out_id: *mut u16,
    ) -> AmmStatus {
        // SAFETY: the link passes label_len and mark_len readable bytes for the call
        let (label, mark) = unsafe { (Self::text(label, label_len), Self::text(mark, mark_len)) };
        let id = Self::with(|state| {
            state.menu_entries.push((label, mark, screens));
            state.menu_entry_id
        });
        // SAFETY: the link passes a writable out pointer for the call
        unsafe { out_id.write(id) };
        AmmStatus::Ok
    }

    unsafe extern "C" fn texture_budget(_: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus {
        let grant = Self::with(|state| state.texture_grant);
        // SAFETY: the client passes writable out pointers for the call
        unsafe {
            out_bytes.write(grant.bytes);
            out_count.write(grant.count);
        }
        AmmStatus::Ok
    }

    unsafe extern "C" fn game_picture(_: AmmHandle, out_width: *mut u32, out_height: *mut u32) -> AmmStatus {
        let (status, width, height) = Self::with(|state| state.game_picture);
        if status == AmmStatus::Ok {
            // SAFETY: the client passes two out pointers that live for the call
            unsafe {
                out_width.write(width);
                out_height.write(height);
            }
        }
        status
    }

    unsafe extern "C" fn set_game_viewport(_: AmmHandle, rect: *const [f32; 4]) -> AmmStatus {
        // SAFETY: the client passes a rect that lives for the call, or null
        let rect = unsafe { rect.as_ref() }.copied();
        Self::with(|state| state.game_viewports.push(rect));
        AmmStatus::Ok
    }

    unsafe extern "C" fn set_render_size(_: AmmHandle, size: *const AmmRenderSize) -> AmmStatus {
        // SAFETY: the client passes a size that lives for the call, or null
        let size = unsafe { size.as_ref() }.copied();
        Self::with(|state| {
            state.render_sizes.push(size);
            state.render_size_status
        })
    }

    unsafe extern "C" fn render_size(_: AmmHandle, out_state: *mut AmmRenderSizeState) -> AmmStatus {
        let (status, answer) = Self::with(|state| state.render_size_state);
        if status == AmmStatus::Ok {
            // SAFETY: the client passes an out state that lives for the call
            unsafe { out_state.write(answer) };
        }
        status
    }

    unsafe extern "C" fn capture_stream_request(_: AmmHandle, request: *const AmmCaptureStreamRequest) -> AmmStatus {
        // SAFETY: the link passes a request that lives for the call
        let Some(request) = (unsafe { request.as_ref() }) else {
            return AmmStatus::NullPointer;
        };
        Self::with(|state| {
            state.stream_requests.push(request.tag);
            state.stream_present_seqs.push(request.present_seq);
            state.stream_request_status
        })
    }

    unsafe extern "C" fn capture_stream_take(_: AmmHandle, out_frame: *mut AmmCaptureStreamFrame) -> AmmStatus {
        Self::with(|state| {
            let Some((status, mut frame, pixels)) = state.stream_takes.pop_front() else {
                return AmmStatus::Busy;
            };
            state.stream_lent = pixels;
            if !state.stream_lent.is_empty() {
                (frame.pixels, frame.pixels_len) = (state.stream_lent.as_ptr(), state.stream_lent.len());
            }
            if status == AmmStatus::Ok {
                // SAFETY: the client passes an out frame that lives for the call
                unsafe { out_frame.write(frame) };
            }
            status
        })
    }

    unsafe extern "C" fn capture_stream_lend(_: AmmHandle, out_frame: *mut AmmCaptureStreamFrame) -> AmmStatus {
        Self::with(|state| {
            let Some((status, mut frame, pixels)) = state.stream_takes.pop_front() else {
                return AmmStatus::Busy;
            };
            if !pixels.is_empty() {
                // a Vec keeps its heap bytes where they are when it moves into the list
                state.stream_lent_buffers.push(pixels);
                if let Some(buffer) = state.stream_lent_buffers.last() {
                    (frame.pixels, frame.pixels_len) = (buffer.as_ptr(), buffer.len());
                }
            }
            if status == AmmStatus::Ok {
                // SAFETY: the client passes an out frame that lives for the call
                unsafe { out_frame.write(frame) };
            }
            status
        })
    }

    extern "C" fn capture_stream_give_back(_: AmmHandle, lend_id: u64) -> AmmStatus {
        Self::with(|state| {
            state.stream_give_backs.push(lend_id);
            state.stream_give_back_status
        })
    }

    extern "C" fn capture_stream_close(_: AmmHandle) -> AmmStatus {
        Self::with(|state| state.stream_closes += 1);
        AmmStatus::Ok
    }

    unsafe extern "C" fn capture_stream_free(_: AmmHandle, out_free: *mut u32) -> AmmStatus {
        let free = Self::with(|state| state.stream_free);
        // SAFETY: the client passes an out count that lives for the call
        unsafe { out_free.write(free) };
        AmmStatus::Ok
    }

    unsafe extern "C" fn present_count(_: AmmHandle, out_count: *mut u64) -> AmmStatus {
        let (status, count) = Self::with(|state| state.present_count);
        if status == AmmStatus::Ok {
            // SAFETY: the client passes an out count that lives for the call
            unsafe { out_count.write(count) };
        }
        status
    }

    unsafe extern "C" fn set_icon(_: AmmHandle, _: *const u8, png_len: usize) -> AmmStatus {
        Self::with(|state| {
            state.icon_calls.push(png_len);
            state.icon_status
        })
    }

    unsafe extern "C" fn watch_keys(_: AmmHandle, vks: *const u32, count: usize) -> AmmStatus {
        let keys = if vks.is_null() || count == 0 {
            Vec::new()
        } else {
            // SAFETY: the client passes count readable u32 values for the call
            unsafe { std::slice::from_raw_parts(vks, count) }.to_vec()
        };
        Self::with(|state| {
            state.watched_keys = keys;
            state.watch_keys_status
        })
    }

    unsafe extern "C" fn send_key(_: AmmHandle, vk: u32, down: u8) -> AmmStatus {
        Self::with(|state| {
            state.sent_key_calls.push((vk, down == 1));
            state.send_key_status
        })
    }

    #[allow(clippy::too_many_arguments)]
    unsafe extern "C" fn log(
        _: AmmHandle,
        level: u8,
        target: *const u8,
        target_len: usize,
        message: *const u8,
        message_len: usize,
        fields_json: *const u8,
        fields_len: usize,
        file: *const u8,
        file_len: usize,
        line: u32,
    ) {
        // SAFETY: the client passes each text for the call
        let call = unsafe {
            LoggedCall {
                level,
                target: Self::text(target, target_len),
                message: Self::text(message, message_len),
                fields_json: Self::text(fields_json, fields_len),
                file: Self::text(file, file_len),
                line,
            }
        };
        Self::with(|state| state.logs.push(call));
    }
}
