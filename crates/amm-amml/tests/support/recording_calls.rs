use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::Duration;

use amm_abi::types::AmmStatus;
use amm_core::structs::input::FakeKeyOp;
use amm_ffi::host::{amm_host_create, amm_host_destroy, amm_host_release_fake_keys, amm_host_tick};
use amm_overlay::{PointerButton, ShutdownOutcome, UninstallOutcome};
use amml_modmanager::ManagerPaths;
use amml_modmanager::services::ManagerState;
use amml_modmanager::traits::GameCalls;
use amml_modmanager::types::{HookGroup, RenderRestore};
use amml_msvc::{MsvcHeap, MsvcString};
use amml_sdk::SdkError;
use amml_sdk::amml_abi::AmmlLogLevel;

/// One call the manager made, with the stop latch where its order matters. Queries of game state are not
/// recorded, only calls that change something or whose order matters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Call {
    CreateHost,
    SetConsole(bool),
    DestroyHost,
    HostTick(u64),
    HostWorld(bool),
    HostKey { vk: u32, down: bool, repeat: bool },
    ReleaseFakeKeys,
    GameKey { controller: usize, vk: u32, down: bool, origin: i32 },
    PointerDelta(i32, i32),
    InstallHooks { latch_raised: bool },
    RemoveHooks(HookGroup),
    InstallOverlay,
    UninstallWndproc,
    WindowForeground,
    ShutdownOverlay,
    CloseRenderSize,
    PostRenderRestore(RenderRestore),
    ReleaseMouse(usize),
    GrabMouse { game: usize, latch_raised: bool },
    ReleaseString,
    SetHideGui(bool),
    Log(String),
}

/// A `GameCalls` that records every call. With `real_host` the host calls go to the `amm_host_*` API of this
/// process; otherwise they only record. The game, the overlay and the router answer with the values set on it.
pub struct RecordingCalls {
    state: &'static ManagerState,
    real_host: bool,
    pub overlay: Mutex<ShutdownOutcome>,
    pub router: Mutex<UninstallOutcome>,
    pub restore: Mutex<Option<RenderRestore>>,
    pub panic_on_tick: AtomicBool,
    pub fail_hooks: AtomicBool,
    pub consume_keys: AtomicBool,
    pub consume_deltas: AtomicBool,
    pub consumed_buttons: Mutex<Vec<PointerButton>>,
    pub focus_lost: AtomicBool,
    pub blocks_turn: AtomicBool,
    pub game_pointer: Mutex<Option<(i16, i16)>>,
    pub foreground: AtomicBool,
    pub local_player: AtomicUsize,
    pub world_active: AtomicBool,
    pub showing_menu: AtomicBool,
    pub screen_name: Mutex<Option<String>>,
    pub options: Mutex<Option<usize>>,
    pub player_hides_hud: AtomicBool,
    fake_keys: Mutex<Vec<FakeKeyOp>>,
    now: AtomicU64,
    calls: Mutex<Vec<Call>>,
}

unsafe extern "C" fn fake_new(size: usize) -> *mut u8 {
    Box::leak(vec![0u8; size].into_boxed_slice()).as_mut_ptr()
}

unsafe extern "C" fn fake_delete(_ptr: *mut u8, _size: usize) {}

impl RecordingCalls {
    pub const CLIENT: usize = 0x1000;
    pub const PLAYER: usize = 0x3000;
    pub const OPTIONS: usize = 0x4000;

    pub fn leaked(state: &'static ManagerState, real_host: bool) -> &'static Self {
        Box::leak(Box::new(Self {
            state,
            real_host,
            overlay: Mutex::new(ShutdownOutcome::Drained),
            router: Mutex::new(UninstallOutcome::Restored),
            restore: Mutex::new(None),
            panic_on_tick: AtomicBool::new(false),
            fail_hooks: AtomicBool::new(false),
            consume_keys: AtomicBool::new(false),
            consume_deltas: AtomicBool::new(false),
            consumed_buttons: Mutex::new(Vec::new()),
            focus_lost: AtomicBool::new(false),
            blocks_turn: AtomicBool::new(false),
            game_pointer: Mutex::new(None),
            foreground: AtomicBool::new(true),
            local_player: AtomicUsize::new(0),
            world_active: AtomicBool::new(false),
            showing_menu: AtomicBool::new(false),
            screen_name: Mutex::new(None),
            options: Mutex::new(Some(Self::OPTIONS)),
            player_hides_hud: AtomicBool::new(false),
            fake_keys: Mutex::new(Vec::new()),
            now: AtomicU64::new(0),
            calls: Mutex::new(Vec::new()),
        }))
    }

    pub fn set_now(&self, now_ms: u64) {
        self.now.store(now_ms, Ordering::SeqCst);
    }

    /// The player is in a world with no screen (`true`), or on a menu screen of one.
    pub fn set_in_play(&self, in_play: bool) {
        self.local_player.store(Self::PLAYER, Ordering::SeqCst);
        self.world_active.store(true, Ordering::SeqCst);
        self.showing_menu.store(!in_play, Ordering::SeqCst);
    }

    /// Fake key presses the host holds for the next `take_fake_keys`.
    pub fn queue_fake_keys(&self, ops: &[FakeKeyOp]) {
        self.fake_keys.lock().unwrap().extend_from_slice(ops);
    }

    /// Fake key presses the host still holds.
    pub fn pending_fake_keys(&self) -> Vec<FakeKeyOp> {
        self.fake_keys.lock().unwrap().clone()
    }

    /// The calls since the last `take`, left in place.
    pub fn seen(&self) -> Vec<Call> {
        self.calls.lock().unwrap().clone()
    }

    /// The calls since the last `take`.
    pub fn take(&self) -> Vec<Call> {
        std::mem::take(&mut *self.calls.lock().unwrap())
    }

    fn record(&self, call: Call) {
        self.calls.lock().unwrap().push(call);
    }
}

impl GameCalls for RecordingCalls {
    fn now_ms(&self) -> u64 {
        self.now.load(Ordering::SeqCst)
    }

    fn create_host(&self, paths: &ManagerPaths) -> AmmStatus {
        self.record(Call::CreateHost);
        if !self.real_host {
            return AmmStatus::Ok;
        }
        let config = paths.config_dir.to_str().unwrap();
        let game = paths.game_dir.to_str().unwrap();
        let version = paths.version.as_str();
        // SAFETY: the three texts live for the call
        unsafe {
            amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
        }
    }

    fn set_console(&self, on: bool) {
        self.record(Call::SetConsole(on));
    }

    fn destroy_host(&self) {
        self.record(Call::DestroyHost);
        if self.real_host {
            amm_host_destroy();
        }
    }

    fn host_tick(&self, now_ms: u64) {
        self.record(Call::HostTick(now_ms));
        if self.panic_on_tick.load(Ordering::SeqCst) {
            panic!("test tick panics");
        }
        if self.real_host {
            amm_host_tick(now_ms);
        }
    }

    fn host_world(&self, loaded: bool) {
        self.record(Call::HostWorld(loaded));
    }

    fn host_key(&self, vk: u32, down: bool, repeat: bool, _mods: u8, _now_ms: u64) -> bool {
        self.record(Call::HostKey { vk, down, repeat });
        self.consume_keys.load(Ordering::SeqCst)
    }

    fn take_fake_keys(&self, out: &mut Vec<FakeKeyOp>) {
        out.append(&mut self.fake_keys.lock().unwrap());
    }

    fn release_fake_keys(&self) {
        self.record(Call::ReleaseFakeKeys);
        if self.real_host {
            amm_host_release_fake_keys();
        }
    }

    fn game_key(&self, controller: usize, vk: u32, down: bool, origin: i32) {
        self.record(Call::GameKey { controller, vk, down, origin });
    }

    fn host_pointer_delta(&self, dx: f32, dy: f32) -> bool {
        self.record(Call::PointerDelta(dx as i32, dy as i32));
        self.consume_deltas.load(Ordering::SeqCst)
    }

    fn blocks_player_turn(&self) -> bool {
        self.blocks_turn.load(Ordering::SeqCst)
    }

    fn modifiers(&self) -> u8 {
        0
    }

    fn take_focus_lost(&self) -> bool {
        self.focus_lost.swap(false, Ordering::SeqCst)
    }

    fn button_consumed(&self, button: PointerButton) -> bool {
        self.consumed_buttons.lock().unwrap().contains(&button)
    }

    fn game_pointer(&self) -> Option<(i16, i16)> {
        *self.game_pointer.lock().unwrap()
    }

    fn game_window_foreground(&self) -> bool {
        self.record(Call::WindowForeground);
        self.foreground.load(Ordering::SeqCst)
    }

    fn install_hooks(&self) -> Result<(), SdkError> {
        self.record(Call::InstallHooks { latch_raised: self.state.latch().raised() });
        if self.fail_hooks.load(Ordering::SeqCst) {
            return Err(SdkError::MissingEntry("ClientInstance.update".to_string()));
        }
        Ok(())
    }

    fn remove_hooks(&self, group: HookGroup) {
        self.record(Call::RemoveHooks(group));
    }

    fn install_overlay(&self) {
        self.record(Call::InstallOverlay);
    }

    fn uninstall_wndproc(&self) -> UninstallOutcome {
        self.record(Call::UninstallWndproc);
        *self.router.lock().unwrap()
    }

    fn shutdown_overlay(&self, timeout: Duration) -> ShutdownOutcome {
        assert_eq!(timeout, Duration::from_millis(2000));
        self.record(Call::ShutdownOverlay);
        *self.overlay.lock().unwrap()
    }

    fn close_render_size(&self) -> Option<RenderRestore> {
        self.record(Call::CloseRenderSize);
        *self.restore.lock().unwrap()
    }

    fn post_render_restore(&self, restore: RenderRestore) {
        self.record(Call::PostRenderRestore(restore));
    }

    fn local_player(&self, _client: usize) -> usize {
        self.local_player.load(Ordering::SeqCst)
    }

    fn world_active(&self, _client: usize) -> bool {
        self.world_active.load(Ordering::SeqCst)
    }

    fn showing_menu(&self, _client: usize) -> bool {
        self.showing_menu.load(Ordering::SeqCst)
    }

    fn release_mouse(&self, game: usize) {
        self.record(Call::ReleaseMouse(game));
    }

    fn grab_mouse(&self, game: usize) {
        self.record(Call::GrabMouse { game, latch_raised: self.state.latch().raised() });
    }

    fn top_screen_name(&self, _client: usize) -> Option<MsvcString> {
        let name = self.screen_name.lock().unwrap().clone()?;
        let heap = MsvcHeap::new(fake_new, fake_delete);
        // SAFETY: the fake heap functions have the operator new and delete signatures
        Some(unsafe { MsvcString::new_in(&heap, &name) }.unwrap())
    }

    fn release_string(&self, _text: MsvcString) {
        self.record(Call::ReleaseString);
    }

    fn options(&self, _client: usize) -> Option<usize> {
        *self.options.lock().unwrap()
    }

    fn hide_hud(&self, _options: usize) -> bool {
        self.player_hides_hud.load(Ordering::SeqCst)
    }

    fn set_hide_gui(&self, _options: usize, hide: bool) {
        self.record(Call::SetHideGui(hide));
    }

    fn log(&self, _level: AmmlLogLevel, message: &str) {
        self.record(Call::Log(message.to_string()));
    }
}
