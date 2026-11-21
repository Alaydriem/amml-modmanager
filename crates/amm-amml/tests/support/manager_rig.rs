use std::cell::Cell;
use std::path::PathBuf;

use amm_abi::types::{
    AmmAcquireOutcome, AmmCapability, AmmCoreTable, AmmEvent, AmmEventKind, AmmHandle, AmmLayersTable, AmmLeasesTable,
    AmmRenderSize, AmmSettingsTable, AmmStatus,
};
use amml_api::amml_abi::{
    AmmlCursorShape, AmmlDirKind, AmmlDrawSurface, AmmlModifiers, AmmlMouseButton, AmmlPoint, AmmlPointerKind,
    AmmlSurfaceLoss, AmmlWindowChange, ClientTick, FramePresent, FramePresented, InjectedInputReleased,
    InputLeaseResumed, InputLeaseSuspended, KeyInput, ModsLoaded, MouseGrab, MouseInput, RenderDeviceReset, Shutdown,
    SurfaceLost, SwapChainResize, SwapChainResized, WindowChanged, WindowChar, WindowCursor, WindowFocus,
    WindowPointer, WindowSize, WorldJoined, WorldLeft,
};
use amml_api::testing::{FakeClient, FakeDispatch, FakeGame, FakeWindow};
use amml_api::{Area, KeyCode, Screen};
use amml_modmanager::ManagerMod;
use amml_modmanager::services::ManagerRuntime;
use tempfile::TempDir;

/// A fake loader set up for the manager: its folders in a temporary AMML layout (`mods/@amml/modmanager/config`,
/// `data/@amml/modmanager`, `game`), every event and area the manager uses offered, and helpers that drive the
/// keys, the ticks and the manager tables as a game and a consumer mod would.
pub struct ManagerRig {
    pub fake: FakeGame,
    dir: TempDir,
    next_mod: Cell<u32>,
}

impl ManagerRig {
    pub const F10: u16 = 0x79;
    pub const CTRL: u16 = 0x11;
    pub const SHIFT: u16 = 0x10;
    pub const LCONTROL: u16 = 0xA2;
    pub const LSHIFT: u16 = 0xA0;
    pub const G: u16 = 0x47;

    /// A rig that offers every event and area the manager uses. Nothing is loaded yet.
    pub fn new() -> Self {
        let rig = Self::without_events();
        rig.fake.offer::<ClientTick>();
        rig.offer_all_but_tick();
        rig
    }

    /// A rig with the folders set and nothing offered.
    pub fn without_events() -> Self {
        let fake = FakeGame::new();
        let dir = tempfile::tempdir().unwrap();
        let rig = Self { fake, dir, next_mod: Cell::new(0) };
        rig.fake.set_dir(AmmlDirKind::CONFIG, &rig.config_dir().display().to_string());
        rig.fake.set_dir(
            AmmlDirKind::DATA,
            &rig.dir.path().join("data").join("@amml").join("modmanager").display().to_string(),
        );
        rig.fake.set_dir(AmmlDirKind::INSTANCE, &rig.dir.path().join("game").display().to_string());
        rig
    }

    /// Offers every event and area the manager uses, except `ClientTick`.
    pub fn offer_all_but_tick(&self) {
        self.fake.offer::<WorldJoined>();
        self.fake.offer::<WorldLeft>();
        self.fake.offer::<ModsLoaded>();
        self.fake.offer::<Shutdown>();
        self.fake.offer::<KeyInput>();
        self.fake.offer::<MouseInput>();
        self.fake.offer::<MouseGrab>();
        self.fake.offer::<InjectedInputReleased>();
        self.fake.offer::<InputLeaseSuspended>();
        self.fake.offer::<InputLeaseResumed>();
        self.fake.offer::<FramePresent>();
        self.fake.offer::<FramePresented>();
        self.fake.offer::<SwapChainResize>();
        self.fake.offer::<SwapChainResized>();
        self.fake.offer::<SurfaceLost>();
        self.fake.offer::<RenderDeviceReset>();
        self.fake.offer::<WindowPointer>();
        self.fake.offer::<WindowChar>();
        self.fake.offer::<WindowFocus>();
        self.fake.offer::<WindowSize>();
        self.fake.offer::<WindowChanged>();
        self.fake.offer::<WindowCursor>();
        for area in [
            Area::CLIENT_READ,
            Area::CLIENT_TOP_SCREEN,
            Area::CLIENT_HUD,
            Area::CLIENT_MOUSE,
            Area::KEY_INJECT,
            Area::WINDOW_READ,
            Area::WINDOW_REQUEST_SIZE,
            Area::OVERLAY_SURFACE,
            Area::OVERLAY_SCREENSHOT,
        ] {
            self.fake.offer_area(area);
        }
        self.fake.set_window(FakeWindow::default());
    }

    /// Loads and enables the manager, opens the game gate, puts the player in play and binds the input feeds.
    pub fn start(&self) {
        self.fake.load::<ManagerMod>().unwrap();
        self.fake.enable::<ManagerMod>().unwrap();
        self.fake.open_gate();
        self.fake.set_client(Self::in_play());
        self.fake.bind_input();
    }

    /// A player in a world with no screen on top, the HUD shown and the mouse held by the game.
    pub fn in_play() -> FakeClient {
        FakeClient {
            local_player: true,
            world_active: true,
            showing_menu: false,
            top_screen: Screen::Other,
            hide_hud: false,
            mouse_captured: Some(true),
            pause_on_focus_lost: true,
            player_alive: Some(true),
            ..FakeClient::default()
        }
    }

    /// The manager's own config folder.
    pub fn config_dir(&self) -> PathBuf {
        self.dir.path().join("mods").join("@amml").join("modmanager").join("config")
    }

    /// One real key; true when the game gets it.
    pub fn key(&self, key: u16, down: bool) -> bool {
        self.fake.real_key(key, down)
    }

    /// Presses and releases the manager chord, Ctrl+Shift+F10.
    pub fn toggle_panel(&self) {
        self.chord(&[Self::CTRL, Self::SHIFT], Self::F10);
    }

    /// Presses the modifiers, presses and releases `key`, then releases the modifiers.
    pub fn chord(&self, modifiers: &[u16], key: u16) {
        for modifier in modifiers {
            self.key(*modifier, true);
        }
        self.key(key, true);
        self.key(key, false);
        for modifier in modifiers.iter().rev() {
            self.key(*modifier, false);
        }
    }

    /// One `ClientTick` with the fake client.
    pub fn tick(&self) -> FakeDispatch {
        self.fake.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() })
    }

    /// A `KeyInput` of `key` raised by the game, not blocked by any lease.
    pub fn raise_key(&self, key: KeyCode, down: bool) -> FakeDispatch {
        let mut event = KeyInput {
            header: FakeGame::header::<KeyInput>(),
            key: key.0,
            down: u8::from(down),
            repeat: 0,
            modifiers: AmmlModifiers(0),
        };
        self.fake.raise_with_client(&mut event)
    }

    pub fn core(&self) -> &'static AmmCoreTable {
        self.fake.ctx().registry().require::<AmmCoreTable>().unwrap()
    }

    pub fn settings(&self) -> &'static AmmSettingsTable {
        self.fake.ctx().registry().require::<AmmSettingsTable>().unwrap()
    }

    pub fn leases(&self) -> &'static AmmLeasesTable {
        self.fake.ctx().registry().require::<AmmLeasesTable>().unwrap()
    }

    /// The core table's `panel_open`, as a consumer reads it.
    pub fn panel_open(&self) -> u8 {
        // SAFETY: panel_open takes no arguments
        unsafe { (self.core().panel_open.unwrap())() }
    }

    /// Whether the manager holds its panel input lease.
    pub fn panel_lease_held(&self) -> bool {
        ManagerRuntime::get().is_some_and(ManagerRuntime::panel_lease_held)
    }

    /// Registers `descriptor` through the core table, in a new config folder inside the mods folder.
    pub fn register(&self, descriptor: &str) -> AmmHandle {
        let n = self.next_mod.get();
        self.next_mod.set(n + 1);
        let config_dir = self.dir.path().join("mods").join("@test").join(format!("mod-{n}")).join("config");
        let dir_text = config_dir.display().to_string();
        let mut handle = AmmHandle { slot: u16::MAX, generation: u16::MAX };
        let register = self.core().register.unwrap();
        // SAFETY: the descriptor, the folder text and the handle live for the call
        let status =
            unsafe { register(descriptor.as_ptr(), descriptor.len(), dir_text.as_ptr(), dir_text.len(), &mut handle) };
        assert_eq!(status, AmmStatus::Ok, "register");
        handle
    }

    /// Every event queued for `handle`.
    pub fn poll(&self, handle: AmmHandle) -> Vec<AmmEvent> {
        let empty =
            AmmEvent { kind: AmmEventKind::Enabled, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 };
        let mut buffer = vec![empty; 64];
        let poll = self.core().poll_events.unwrap();
        // SAFETY: the buffer has room for its length in events for the call
        let count = unsafe { poll(handle, buffer.as_mut_ptr(), buffer.len()) };
        buffer.truncate(count);
        buffer
    }

    /// The `Keybind` events queued for `handle`, as `(index, phase)`.
    pub fn keybinds(&self, handle: AmmHandle) -> Vec<(u32, u8)> {
        self.poll(handle)
            .into_iter()
            .filter(|event| event.kind == AmmEventKind::Keybind)
            .map(|event| (event.code, event.phase))
            .collect()
    }

    /// Takes the `Hud` lease for `handle` through the leases table; true when it is granted.
    pub fn acquire_hud(&self, handle: AmmHandle) -> bool {
        // SAFETY: acquire takes plain values
        let result = unsafe { (self.leases().acquire.unwrap())(handle, AmmCapability::Hud as u8) };
        result.outcome == AmmAcquireOutcome::Granted
    }

    pub fn release_hud(&self, handle: AmmHandle) {
        // SAFETY: release takes plain values
        unsafe { (self.leases().release.unwrap())(handle, AmmCapability::Hud as u8) };
    }

    /// `set_setting` of the settings table for `handle`.
    pub fn set_setting(&self, handle: AmmHandle, path: &str, value: &str) -> AmmStatus {
        let set_setting = self.settings().set_setting.unwrap();
        // SAFETY: both texts live for the call
        unsafe { set_setting(handle, path.as_ptr(), path.len(), value.as_ptr(), value.len()) }
    }

    /// How many log lines of the mod start with `prefix`.
    pub fn log_count(&self, prefix: &str) -> usize {
        self.fake.logs().iter().filter(|line| line.starts_with(prefix)).count()
    }

    /// The first log line that starts with `prefix`.
    pub fn log_line(&self, prefix: &str) -> Option<String> {
        self.fake.logs().into_iter().find(|line| line.starts_with(prefix))
    }

    pub fn layers(&self) -> &'static AmmLayersTable {
        self.fake.ctx().registry().require::<AmmLayersTable>().unwrap()
    }

    /// Takes the lease of `capability` for `handle`; true when it is granted.
    pub fn acquire(&self, handle: AmmHandle, capability: AmmCapability) -> bool {
        // SAFETY: acquire takes plain values
        let result = unsafe { (self.leases().acquire.unwrap())(handle, capability as u8) };
        result.outcome == AmmAcquireOutcome::Granted
    }

    /// Gives `handle` the counting callbacks of `ConsumerCallbacks`.
    pub fn set_consumer_callbacks(&self, handle: AmmHandle) {
        let callbacks = super::ConsumerCallbacks::table();
        // SAFETY: the callbacks live for the call; the table copies them
        let status = unsafe { (self.layers().set_callbacks.unwrap())(handle, &callbacks) };
        assert_eq!(status, AmmStatus::Ok, "set_callbacks");
    }

    /// Forces `width` x `height` for `handle`, or ends the size it forces with `None`.
    pub fn force_size(&self, handle: AmmHandle, size: Option<(u32, u32)>) -> AmmStatus {
        let size = size.map(|(width, height)| AmmRenderSize { width, height });
        let pointer = size.as_ref().map_or(std::ptr::null(), std::ptr::from_ref);
        // SAFETY: the size lives for the call
        unsafe { (self.layers().set_render_size.unwrap())(handle, pointer) }
    }

    /// One present of `surface`: the manager's listeners draw on it.
    pub fn present(&self, surface: AmmlDrawSurface) -> FakeDispatch {
        self.fake.raise(&mut FramePresent {
            header: FakeGame::header::<FramePresent>(),
            frame_id: 1,
            sync_interval: 1,
            flags: 0,
            surface,
        })
    }

    /// A surface the loader does not have: no device yet, or a minimised window.
    pub fn no_surface() -> AmmlDrawSurface {
        AmmlDrawSurface {
            size: 0,
            version: 0,
            width: 0,
            height: 0,
            format: 0,
            wrapped: 0,
            _reserved: [0; 3],
            device_epoch: 0,
            device: std::ptr::null_mut(),
            context: std::ptr::null_mut(),
            target: std::ptr::null_mut(),
            back_buffer: std::ptr::null_mut(),
        }
    }

    /// `SurfaceLost` of device epoch `epoch`.
    pub fn lose_surface(&self, epoch: u64) -> FakeDispatch {
        self.fake.raise(&mut SurfaceLost {
            header: FakeGame::header::<SurfaceLost>(),
            reason: AmmlSurfaceLoss::DEVICE_CHANGED,
            _reserved: [0; 7],
            device_epoch: epoch,
        })
    }

    /// One `WindowPointer` at `client` in client pixels and `buffer` in buffer pixels.
    pub fn pointer(
        &self,
        kind: AmmlPointerKind,
        button: AmmlMouseButton,
        down: bool,
        client: (f32, f32),
        buffer: (f32, f32),
    ) -> FakeDispatch {
        self.fake.raise(&mut WindowPointer {
            header: FakeGame::header::<WindowPointer>(),
            kind,
            button,
            down: u8::from(down),
            _reserved: 0,
            modifiers: AmmlModifiers(0),
            _reserved2: 0,
            client: AmmlPoint { x: client.0, y: client.1 },
            buffer: AmmlPoint { x: buffer.0, y: buffer.1 },
            notches: 0.0,
        })
    }

    /// A move to `at`, then a left press and release there, with client and buffer pixels the same. Gives whether the
    /// press was cancelled.
    pub fn click(&self, at: (f32, f32)) -> bool {
        self.pointer(AmmlPointerKind::MOVE, AmmlMouseButton::LEFT, false, at, at);
        let press = self.pointer(AmmlPointerKind::BUTTON, AmmlMouseButton::LEFT, true, at, at);
        self.pointer(AmmlPointerKind::BUTTON, AmmlMouseButton::LEFT, false, at, at);
        press.cancelled
    }

    /// One `WindowFocus` message: a gain, a loss or a replayed loss.
    pub fn focus(&self, gained: bool, replayed: bool) -> FakeDispatch {
        self.fake.raise(&mut WindowFocus {
            header: FakeGame::header::<WindowFocus>(),
            gained: u8::from(gained),
            replayed: u8::from(replayed),
            _reserved: [0; 2],
        })
    }

    /// One `WindowCursor` over the client area; gives the shape the listeners left in it.
    pub fn cursor(&self) -> AmmlCursorShape {
        let mut event = WindowCursor {
            header: FakeGame::header::<WindowCursor>(),
            shape: AmmlCursorShape::NONE,
            _reserved: [0; 3],
        };
        self.fake.raise(&mut event);
        event.shape
    }

    /// One `WM_SIZE` of `width` x `height`, posted by the loader when `injected`; gives the size the game gets.
    pub fn window_size(&self, width: u32, height: u32, injected: bool) -> (u32, u32) {
        let mut header = FakeGame::header::<WindowSize>();
        header.injected = u8::from(injected);
        let mut event = WindowSize { header, minimised: 0, _reserved: [0; 3], width, height };
        self.fake.raise(&mut event);
        (event.width, event.height)
    }

    /// One `ResizeBuffers` of the bound chain asking `width` x `height`; gives the size it resizes to.
    pub fn resize(&self, width: u32, height: u32) -> (u32, u32) {
        let mut event =
            SwapChainResize { header: FakeGame::header::<SwapChainResize>(), width, height, buffer_count: 2 };
        self.fake.raise(&mut event);
        (event.width, event.height)
    }

    /// The `SwapChainResized` after a resize.
    pub fn resized(&self, ok: bool, width: u32, height: u32) -> FakeDispatch {
        self.fake.raise(&mut SwapChainResized {
            header: FakeGame::header::<SwapChainResized>(),
            ok: u8::from(ok),
            _reserved: [0; 3],
            width,
            height,
        })
    }

    /// `FramePresented` of the last present.
    pub fn presented(&self) -> FakeDispatch {
        self.fake.raise(&mut FramePresented {
            header: FakeGame::header::<FramePresented>(),
            frame_id: 1,
            result: 0,
            cpu_us: 100,
        })
    }

    /// `RenderDeviceReset` for device epoch `epoch`.
    pub fn device_reset(&self, epoch: u64) -> FakeDispatch {
        self.fake.raise(&mut RenderDeviceReset { header: FakeGame::header::<RenderDeviceReset>(), device_epoch: epoch })
    }

    /// `WindowChanged` with a DPI change.
    pub fn window_changed(&self) -> FakeDispatch {
        self.fake.raise(&mut WindowChanged {
            header: FakeGame::header::<WindowChanged>(),
            kind: AmmlWindowChange::DPI,
            _reserved: [0; 3],
        })
    }
}
