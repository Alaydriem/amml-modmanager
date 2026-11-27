use std::cell::RefCell;

use amm_abi::types::{
    AMM_ABI_MINOR, AmmAcquireOutcome, AmmAcquireResult, AmmCallbacks, AmmCapability, AmmCoreTable, AmmEvent,
    AmmFrameInput, AmmFrameOutput, AmmHandle, AmmKeybindsTable, AmmLayersTable, AmmLeasesTable, AmmRenderSize,
    AmmRenderSizeState, AmmSettingsTable, AmmStatus,
};
use amm_core::structs::descriptor::{Capability, Descriptor};
use amm_core::structs::event::ModEvent;
use amm_core::structs::lease::AcquireOutcome;
use amm_core::traits::AbiSized;
use amml_api::amml_abi::{
    AmmlDirKind, AmmlStr, InjectedInputReleased, InputLeaseLost, InputLeaseResumed, InputLeaseSuspended, ModsLoaded,
    ProviderGone, ProviderReady, ScreenshotTaken, ScreenshotWritten,
};
use amml_api::testing::FakeGame;
use amml_api::{ApiError, Area, Ctx, Provided};

#[cfg(feature = "egui")]
use crate::link::FrameHandler;
use crate::link::{ManagerLink, ManagerTables};

use super::{FakeConnection, FakeState, SettingCall};

thread_local! {
    static STATE: RefCell<FakeState> = RefCell::new(FakeState::new());
}

/// The manager's five registry tables for a client's tests, as statics, the way the manager provides them. Each
/// function records its call in a `FakeState` of the calling thread and answers from it. The tables are provided
/// into a `FakeGame`, whose registry the link requires them from; `FakeGame` has one mod, so the provider and the
/// consumer are the same mod there, and a test raises `ProviderReady` and `ProviderGone` itself.
pub struct FakeTable;

impl FakeTable {
    pub const CONFIG_DIR: &'static str = "C:\\mods\\@test\\alaydriem-test\\config";
    pub const DESCRIPTOR: &'static str = "namespace = \"test\"\nname = \"alaydriem-test\"\n\
display_name = \"Test\"\nversion = \"0.1.0\"\nauthor = \"Alaydriem\"\nconfig_path = \"config.hcl\"\n";

    pub const CORE: AmmCoreTable = AmmCoreTable {
        size: size_of::<AmmCoreTable>() as u32,
        version: AMM_ABI_MINOR,
        register: Some(Self::register),
        unregister: Some(Self::unregister),
        poll_events: Some(Self::poll_events),
        locale: Some(Self::locale),
        panel_open: Some(Self::panel_open),
        register_menu_entry: Some(Self::register_menu_entry),
        set_icon: Some(Self::set_icon),
    };
    pub const SETTINGS: AmmSettingsTable = AmmSettingsTable {
        size: size_of::<AmmSettingsTable>() as u32,
        version: AMM_ABI_MINOR,
        config_document: Some(Self::config_document),
        set_setting: Some(Self::set_setting),
    };
    pub const KEYBINDS: AmmKeybindsTable = AmmKeybindsTable {
        size: size_of::<AmmKeybindsTable>() as u32,
        version: AMM_ABI_MINOR,
        chord_text: Some(Self::chord_text),
        bind_down: Some(Self::bind_down),
    };
    pub const LAYERS: AmmLayersTable = AmmLayersTable {
        size: size_of::<AmmLayersTable>() as u32,
        version: AMM_ABI_MINOR,
        set_callbacks: Some(Self::set_callbacks),
        texture_budget: Some(Self::texture_budget),
        set_cursor_mode: Some(Self::set_cursor_mode),
        game_picture: Some(Self::game_picture),
        set_game_viewport: Some(Self::set_game_viewport),
        set_render_size: Some(Self::set_render_size),
        render_size: Some(Self::render_size),
    };
    pub const LEASES: AmmLeasesTable = AmmLeasesTable {
        size: size_of::<AmmLeasesTable>() as u32,
        version: AMM_ABI_MINOR,
        acquire: Some(Self::acquire),
        release: Some(Self::release),
        holder: Some(Self::holder),
        held_by_other: Some(Self::held_by_other),
    };

    /// A fake loader that offers every event the link listens to, the input and overlay areas the link uses, with
    /// its input feeds bound and `CONFIG_DIR` as the mod's config folder.
    pub fn game() -> FakeGame {
        let game = FakeGame::new();
        game.offer::<ProviderReady>();
        game.offer::<ProviderGone>();
        game.offer::<ModsLoaded>();
        game.offer::<InputLeaseSuspended>();
        game.offer::<InputLeaseResumed>();
        game.offer::<InputLeaseLost>();
        game.offer::<InjectedInputReleased>();
        game.offer::<ScreenshotTaken>();
        game.offer::<ScreenshotWritten>();
        for area in [
            Area::KEY_INJECT,
            Area::MOUSE_INJECT,
            Area::TURN,
            Area::MOVE,
            Area::OVERLAY_SCREENSHOT,
            Area::OVERLAY_CAPTURE,
        ] {
            game.offer_area(area);
        }
        game.bind_input();
        game.open_gate();
        game.set_dir(AmmlDirKind::CONFIG, Self::CONFIG_DIR);
        game
    }

    /// Provides the five tables into the registry of `ctx`, core first, as the manager does in its `enable`.
    pub fn provide(ctx: &Ctx) -> Result<(), ApiError> {
        static CORE: AmmCoreTable = FakeTable::CORE;
        static SETTINGS: AmmSettingsTable = FakeTable::SETTINGS;
        static KEYBINDS: AmmKeybindsTable = FakeTable::KEYBINDS;
        static LAYERS: AmmLayersTable = FakeTable::LAYERS;
        static LEASES: AmmLeasesTable = FakeTable::LEASES;
        let registry = ctx.registry();
        registry.provide(&CORE)?;
        registry.provide(&SETTINGS)?;
        registry.provide(&KEYBINDS)?;
        registry.provide(&LAYERS)?;
        registry.provide(&LEASES)
    }

    /// Raises `ProviderReady` for each of the five tables, core first, as the loader does after `ModsLoaded`.
    pub fn ready(game: &FakeGame) {
        for name in ManagerTables::NAMES {
            let header = FakeGame::header::<ProviderReady>();
            let minor = AMM_ABI_MINOR as u16;
            let _ = game.raise(&mut ProviderReady {
                header,
                name: AmmlStr::from_str(name),
                major: AmmCoreTable::MAJOR,
                minor,
            });
        }
    }

    /// Raises `ProviderGone` for each of the five tables, as the loader does before it disables the manager.
    pub fn gone(game: &FakeGame) {
        for name in ManagerTables::NAMES {
            let header = FakeGame::header::<ProviderGone>();
            let _ = game.raise(&mut ProviderGone { header, name: AmmlStr::from_str(name), major: AmmCoreTable::MAJOR });
        }
    }

    /// A `Ctx` of `game` whose mod id is the id `descriptor` names, as a link needs; the fake's own id for a
    /// descriptor that does not parse. Each call leaks the id string; a test makes few calls.
    pub fn ctx_of(game: &FakeGame, descriptor: &str) -> Ctx {
        match hcl::from_str::<Descriptor>(descriptor) {
            Ok(parsed) => game.ctx_named(Box::leak(parsed.id().into_boxed_str())),
            Err(_) => game.ctx(),
        }
    }

    /// Raises `ModsLoaded`.
    pub fn mods_loaded(game: &FakeGame) {
        let _ = game.raise(&mut ModsLoaded { header: FakeGame::header::<ModsLoaded>() });
    }

    /// A link connected to the fake manager with `descriptor`, started with `handler` as its egui pass.
    #[cfg(feature = "egui")]
    pub fn link_with<H: FrameHandler + Send + 'static>(descriptor: &'static str, handler: H) -> FakeConnection {
        let game = Self::game();
        let ctx = Self::ctx_of(&game, descriptor);
        let link = match ManagerLink::start(&ctx, descriptor, Some(handler)) {
            Ok(link) => link,
            Err(error) => panic!("start failed: {error}"),
        };
        Self::connect(game, &ctx, link)
    }

    /// A link connected to the fake manager with `DESCRIPTOR` and `handler` as its egui pass.
    #[cfg(feature = "egui")]
    pub fn link<H: FrameHandler + Send + 'static>(handler: H) -> FakeConnection {
        Self::link_with(Self::DESCRIPTOR, handler)
    }

    /// A link with no frame connected to the fake manager with `descriptor`.
    pub fn link_without_frame_with(descriptor: &'static str) -> FakeConnection {
        let game = Self::game();
        let ctx = Self::ctx_of(&game, descriptor);
        let link = match ManagerLink::start_without_frame(&ctx, descriptor) {
            Ok(link) => link,
            Err(error) => panic!("start failed: {error}"),
        };
        Self::connect(game, &ctx, link)
    }

    /// A link with no frame connected to the fake manager with `DESCRIPTOR`.
    pub fn link_without_frame() -> FakeConnection {
        Self::link_without_frame_with(Self::DESCRIPTOR)
    }

    fn connect(game: FakeGame, ctx: &Ctx, link: ManagerLink) -> FakeConnection {
        if let Err(error) = Self::provide(ctx) {
            panic!("provide failed: {error}");
        }
        Self::ready(&game);
        FakeConnection { link, game }
    }

    /// Runs `f` over this thread's fake state.
    pub fn with<R>(f: impl FnOnce(&mut FakeState) -> R) -> R {
        STATE.with(|state| f(&mut state.borrow_mut()))
    }

    /// The table functions called so far, in order.
    pub fn calls() -> Vec<&'static str> {
        Self::with(|state| state.calls.clone())
    }

    /// Queues `event` for the next `poll_events`.
    pub fn push_event(event: ModEvent) {
        let Ok(event) = AmmEvent::try_from(event) else { panic!("{event:?} has no ABI kind") };
        Self::with(|state| state.events.push_back(event));
    }

    /// Sets the text `config_document` returns.
    pub fn set_config(text: &str) {
        Self::with(|state| state.config = text.to_owned());
    }

    /// Sets what `acquire` answers for `capability`. A capability with no outcome set is granted.
    pub fn set_acquire_outcome(capability: Capability, outcome: AcquireOutcome) {
        Self::with(|state| state.acquire_outcomes.insert(capability, outcome));
    }

    /// Runs the frame callback the link gave `set_callbacks` once over an empty input.
    pub fn run_frame() -> i32 {
        let Some(callbacks) = Self::with(|state| state.callbacks) else { panic!("no callbacks were set") };
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
        // SAFETY: the user pointer is the one the link gave, and input and out live for the call
        unsafe { frame(callbacks.user, &input, &mut out) }
    }

    /// A frame output with every pointer null and every count zero.
    pub fn empty_output() -> AmmFrameOutput {
        AmmFrameOutput::blank()
    }

    fn record(name: &'static str) {
        let hook = Self::with(|state| {
            state.calls.push(name);
            state.on_call.take()
        });
        if let Some(mut hook) = hook {
            hook(name);
            Self::with(|state| {
                if state.on_call.is_none() {
                    state.on_call = Some(hook);
                }
            });
        }
    }

    /// Runs `hook` after each table function is recorded, with its name, from inside that function.
    pub fn on_call(hook: impl FnMut(&'static str) + 'static) {
        Self::with(|state| state.on_call = Some(Box::new(hook)));
    }

    unsafe fn text(ptr: *const u8, len: usize) -> String {
        if ptr.is_null() || len == 0 {
            return String::new();
        }
        // SAFETY: the client passes a pointer to len bytes that live for the call
        let bytes = unsafe { std::slice::from_raw_parts(ptr, len) };
        String::from_utf8_lossy(bytes).into_owned()
    }

    unsafe fn copy_out(text: &str, out_buf: *mut u8, out_cap: usize) -> u32 {
        if !text.is_empty() && text.len() <= out_cap {
            // SAFETY: the client passes a buffer with room for out_cap bytes
            unsafe { std::ptr::copy_nonoverlapping(text.as_ptr(), out_buf, text.len()) };
        }
        text.len() as u32
    }

    fn capability(code: u8) -> Option<Capability> {
        AmmCapability::try_from(code).ok().map(Capability::from)
    }

    unsafe extern "C" fn register(
        descriptor: *const u8,
        descriptor_len: usize,
        config_dir: *const u8,
        config_dir_len: usize,
        out_handle: *mut AmmHandle,
    ) -> AmmStatus {
        Self::record("register");
        // SAFETY: the client passes its descriptor and folder texts for the call
        let (descriptor, config_dir) =
            unsafe { (Self::text(descriptor, descriptor_len), Self::text(config_dir, config_dir_len)) };
        Self::with(|state| {
            state.descriptor = Some(descriptor);
            state.config_dir = Some(config_dir);
            if state.register_status == AmmStatus::Ok {
                // SAFETY: the client passes a writable handle for the call
                unsafe { out_handle.write(state.handle) };
            }
            state.register_status
        })
    }

    unsafe extern "C" fn unregister(handle: AmmHandle) -> AmmStatus {
        Self::record("unregister");
        Self::with(|state| {
            state.unregistered.push(handle);
            state.unregister_status
        })
    }

    unsafe extern "C" fn poll_events(_: AmmHandle, out_buf: *mut AmmEvent, capacity: usize) -> usize {
        Self::record("poll_events");
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

    unsafe extern "C" fn locale(_: AmmHandle, out_buf: *mut u8, out_cap: usize) -> u32 {
        Self::record("locale");
        let code = Self::with(|state| state.locale);
        // SAFETY: the client passes a buffer with room for out_cap bytes
        unsafe { Self::copy_out(code, out_buf, out_cap) }
    }

    unsafe extern "C" fn panel_open() -> u8 {
        Self::record("panel_open");
        u8::from(Self::with(|state| state.panel_open))
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
        Self::record("register_menu_entry");
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

    unsafe extern "C" fn set_icon(_: AmmHandle, _: *const u8, png_len: usize) -> AmmStatus {
        Self::record("set_icon");
        Self::with(|state| {
            state.icon_calls.push(png_len);
            state.icon_status
        })
    }

    unsafe extern "C" fn config_document(_: AmmHandle, out_ptr: *mut *const u8, out_len: *mut usize) -> AmmStatus {
        Self::record("config_document");
        // The text stays in this thread's state, unchanged until the test sets it again
        let (ptr, len) = Self::with(|state| (state.config.as_ptr(), state.config.len()));
        // SAFETY: the client passes writable out pointers for the call
        unsafe {
            out_ptr.write(ptr);
            out_len.write(len);
        }
        AmmStatus::Ok
    }

    unsafe extern "C" fn set_setting(
        _: AmmHandle,
        path: *const u8,
        path_len: usize,
        value: *const u8,
        value_len: usize,
    ) -> AmmStatus {
        Self::record("set_setting");
        // SAFETY: the client passes the path and expression texts for the call
        let call =
            unsafe { SettingCall { path: Self::text(path, path_len), expression: Self::text(value, value_len) } };
        Self::with(|state| {
            state.settings.push(call);
            state.set_setting_status
        })
    }

    unsafe extern "C" fn chord_text(_: AmmHandle, _: u32, _: *mut u8, _: usize) -> u32 {
        Self::record("chord_text");
        0
    }

    unsafe extern "C" fn bind_down(_: AmmHandle, _: u32) -> u8 {
        Self::record("bind_down");
        0
    }

    unsafe extern "C" fn set_callbacks(_: AmmHandle, callbacks: *const AmmCallbacks) -> AmmStatus {
        Self::record("set_callbacks");
        Self::with(|state| {
            if state.callbacks_status == AmmStatus::Ok {
                // SAFETY: the client passes live callbacks for the call
                state.callbacks = Some(unsafe { callbacks.read() });
            }
            state.callbacks_status
        })
    }

    unsafe extern "C" fn texture_budget(_: AmmHandle, out_bytes: *mut u64, out_count: *mut u32) -> AmmStatus {
        Self::record("texture_budget");
        let grant = Self::with(|state| state.texture_grant);
        // SAFETY: the client passes writable out pointers for the call
        unsafe {
            out_bytes.write(grant.bytes);
            out_count.write(grant.count);
        }
        AmmStatus::Ok
    }

    unsafe extern "C" fn set_cursor_mode(_: AmmHandle, on: u8) {
        Self::record("set_cursor_mode");
        Self::with(|state| state.cursor_modes.push(on == 1));
    }

    unsafe extern "C" fn game_picture(_: AmmHandle, out_width: *mut u32, out_height: *mut u32) -> AmmStatus {
        Self::record("game_picture");
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
        Self::record("set_game_viewport");
        // SAFETY: the client passes a rect that lives for the call, or null
        let rect = unsafe { rect.as_ref() }.copied();
        Self::with(|state| state.game_viewports.push(rect));
        AmmStatus::Ok
    }

    unsafe extern "C" fn set_render_size(_: AmmHandle, size: *const AmmRenderSize) -> AmmStatus {
        Self::record("set_render_size");
        // SAFETY: the client passes a size that lives for the call, or null
        let size = unsafe { size.as_ref() }.copied();
        Self::with(|state| {
            state.render_sizes.push(size);
            state.render_size_status
        })
    }

    unsafe extern "C" fn render_size(_: AmmHandle, out_state: *mut AmmRenderSizeState) -> AmmStatus {
        Self::record("render_size");
        let (status, answer) = Self::with(|state| state.render_size_state);
        if status == AmmStatus::Ok {
            // SAFETY: the client passes an out state that lives for the call
            unsafe { out_state.write(answer) };
        }
        status
    }

    unsafe extern "C" fn acquire(_: AmmHandle, capability: u8) -> AmmAcquireResult {
        Self::record("acquire");
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
        Self::record("release");
        if let Some(capability) = Self::capability(capability) {
            Self::with(|state| state.released.push(capability));
        }
    }

    unsafe extern "C" fn holder(_: u8, out_buf: *mut u8, out_cap: usize) -> u32 {
        Self::record("holder");
        let id = Self::with(|state| state.holder_id).unwrap_or("");
        // SAFETY: the client passes a buffer with room for out_cap bytes
        unsafe { Self::copy_out(id, out_buf, out_cap) }
    }

    unsafe extern "C" fn held_by_other(_: AmmHandle, capability: u8) -> u8 {
        Self::record("held_by_other");
        let Some(capability) = Self::capability(capability) else {
            return 0;
        };
        u8::from(Self::with(|state| state.held_by_other.get(&capability).copied().unwrap_or(false)))
    }
}
