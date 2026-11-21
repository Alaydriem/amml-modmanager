use amm_client::test_support::FakeTable;
use amml_api::amml_abi::{
    AmmlDirKind, ClientTick, FramePresent, FramePresented, GamepadPoll, InjectedInputReleased, InputLeaseLost,
    InputLeaseResumed, InputLeaseSuspended, KeyInput, LookDelta, ModsLoaded, MouseGrab, MouseInput, PlayerMoveInput,
    ProviderGone, ProviderReady, RenderDeviceReset, ScreenshotTaken, ScreenshotWritten, Shutdown, SurfaceLost,
    SwapChainResize, SwapChainResized, WindowChanged, WindowChar, WindowCursor, WindowFocus, WindowPointer, WindowSize,
    WorldJoined, WorldLeft,
};
use amml_api::testing::{FakeClient, FakeGame, FakeWindow};
use amml_api::{Area, Screen};
use amml_managerprobe::ProbeMod;
use amml_modmanager::ManagerMod;
use tempfile::TempDir;

/// A fake loader with the real manager mod and the probe in it. Its folders are a temporary AMML layout and it
/// offers every event and area either mod uses.
pub struct ProbeRig {
    pub fake: FakeGame,
    dir: TempDir,
}

impl ProbeRig {
    pub const F8: u16 = 0x77;

    /// Nothing is loaded yet.
    pub fn new() -> Self {
        let fake = FakeGame::new();
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("mods").join("@amml").join("modmanager").join("config");
        fake.set_dir(AmmlDirKind::CONFIG, &config.display().to_string());
        fake.set_dir(
            AmmlDirKind::DATA,
            &dir.path().join("data").join("@amml").join("modmanager").display().to_string(),
        );
        fake.set_dir(AmmlDirKind::INSTANCE, &dir.path().join("game").display().to_string());
        let rig = Self { fake, dir };
        rig.offer_all();
        rig
    }

    fn offer_all(&self) {
        self.fake.offer::<ClientTick>();
        self.fake.offer::<WorldJoined>();
        self.fake.offer::<WorldLeft>();
        self.fake.offer::<ModsLoaded>();
        self.fake.offer::<ProviderReady>();
        self.fake.offer::<ProviderGone>();
        self.fake.offer::<Shutdown>();
        self.fake.offer::<KeyInput>();
        self.fake.offer::<MouseInput>();
        self.fake.offer::<MouseGrab>();
        self.fake.offer::<PlayerMoveInput>();
        self.fake.offer::<LookDelta>();
        self.fake.offer::<GamepadPoll>();
        self.fake.offer::<InjectedInputReleased>();
        self.fake.offer::<InputLeaseSuspended>();
        self.fake.offer::<InputLeaseResumed>();
        self.fake.offer::<InputLeaseLost>();
        self.fake.offer::<ScreenshotTaken>();
        self.fake.offer::<ScreenshotWritten>();
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
            Area::MOUSE_INJECT,
            Area::TURN,
            Area::MOVE,
            Area::WINDOW_READ,
            Area::WINDOW_REQUEST_SIZE,
            Area::OVERLAY_SURFACE,
            Area::OVERLAY_SCREENSHOT,
        ] {
            self.fake.offer_area(area);
        }
        self.fake.set_window(FakeWindow::default());
    }

    /// Loads and enables both mods in the loader's order (the probe first: `managerprobe` < `modmanager`), opens the
    /// game gate, puts the player in play and binds the input feeds.
    pub fn start(&self) {
        self.fake.load::<ProbeMod>().unwrap();
        self.fake.load::<ManagerMod>().unwrap();
        self.fake.enable::<ProbeMod>().unwrap();
        self.fake.enable::<ManagerMod>().unwrap();
        self.fake.open_gate();
        self.fake.set_client(FakeClient {
            local_player: true,
            world_active: true,
            showing_menu: false,
            top_screen: Screen::Other,
            hide_hud: false,
            mouse_captured: Some(true),
            pause_on_focus_lost: true,
            player_alive: Some(true),
            ..FakeClient::default()
        });
        self.fake.bind_input();
    }

    /// What the loader does after every mod is enabled: `ModsLoaded`, then a `ProviderReady` per manager table.
    pub fn mods_loaded(&self) {
        // The probe gives the manager its own config folder at connect, and the manager refuses its own
        let config = self.dir.path().join("mods").join("@amml").join("managerprobe").join("config");
        self.fake.set_dir(AmmlDirKind::CONFIG, &config.display().to_string());
        FakeTable::mods_loaded(&self.fake);
        FakeTable::ready(&self.fake);
    }

    /// One `ClientTick` with the fake client.
    pub fn tick(&self) {
        self.fake.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() });
    }

    /// The loader's `ProviderGone` for each manager table, as it comes before the manager is disabled.
    pub fn provider_gone(&self) {
        FakeTable::gone(&self.fake);
    }

    /// Whether some log line is exactly `line`.
    pub fn logged(&self, line: &str) -> bool {
        self.fake.logs().iter().any(|logged| logged == line)
    }
}

impl ProbeRig {
    /// Loads and enables only the manager, as `start` does, and gives the probe its own config folder.
    pub fn start_manager(&self) {
        self.start_manager_unbound();
        self.fake.bind_input();
    }

    /// `start_manager` with the input feeds not bound yet, as before the first real key.
    pub fn start_manager_unbound(&self) {
        self.fake.load::<ManagerMod>().unwrap();
        self.fake.enable::<ManagerMod>().unwrap();
        self.fake.open_gate();
        self.fake.set_client(FakeClient {
            local_player: true,
            world_active: true,
            showing_menu: false,
            top_screen: Screen::Other,
            hide_hud: false,
            mouse_captured: Some(true),
            pause_on_focus_lost: true,
            player_alive: Some(true),
            ..FakeClient::default()
        });
        let config = self.dir.path().join("mods").join("@amml").join("managerprobe").join("config");
        self.fake.set_dir(AmmlDirKind::CONFIG, &config.display().to_string());
    }
}

impl ProbeRig {
    /// Starts the manager and the checks, draws the square, and ticks until the checks wait to hold W. Gives the
    /// checks.
    pub fn checks_at_key_hold(&self) -> std::sync::Arc<amml_managerprobe::ProbeChecks> {
        self.start_manager();
        self.checks_after_start()
    }

    /// `checks_at_key_hold` with the input feeds not bound.
    pub fn checks_at_key_hold_unbound(&self) -> std::sync::Arc<amml_managerprobe::ProbeChecks> {
        self.start_manager_unbound();
        self.checks_after_start()
    }

    /// Ticks the checks through the settle time of the key hold, so the next tick presses W.
    pub fn settle(&self, checks: &amml_managerprobe::ProbeChecks) {
        for _ in 0..amml_managerprobe::ProbeChecks::SETTLE_TICKS {
            checks.tick();
        }
    }

    fn checks_after_start(&self) -> std::sync::Arc<amml_managerprobe::ProbeChecks> {
        self.fake.enable::<super::ChecksMod>().unwrap();
        let (checks, frames) = super::ChecksMod::checks().clone().unwrap();
        frames.store(amml_managerprobe::ProbeLayer::TARGET_FRAMES, std::sync::atomic::Ordering::Relaxed);
        for _ in 0..6 {
            checks.tick();
            self.tick();
        }
        checks
    }
}
