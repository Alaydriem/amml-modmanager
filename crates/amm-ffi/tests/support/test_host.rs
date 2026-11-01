use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_abi::types::{AMM_ABI_MAJOR, AmmApi, AmmStatus};
use amm_ffi::api::amm_api_for;
use amm_ffi::host::{amm_host_create, amm_host_destroy, amm_host_frame, amm_host_key};
use amm_ffi::services::HostCell;
use amm_ffi::services::host_runtime::HostRuntime;
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput, AmmHostLayer, AmmHostRoute};
use tempfile::TempDir;

/// The process-global host, created in a temporary folder and destroyed on drop.
pub struct TestHost {
    dir: TempDir,
    next_mod: AtomicU32,
}

impl TestHost {
    pub const PLAY: u8 = 0;
    pub const START: u8 = 1;
    pub const PAUSE: u8 = 2;
    pub const WIDTH: u32 = 1920;
    pub const HEIGHT: u32 = 1080;
    pub const MANAGER: u8 = 0xFE;
    pub const CTRL: u8 = 1;
    pub const SHIFT: u8 = 2;
    pub const R8G8B8A8_UNORM: u32 = 28;

    pub fn new() -> TestHost {
        Self::with_config("")
    }

    /// A host whose manager `config.hcl` holds `config` before it starts. An empty text writes no file.
    pub fn with_config(config: &str) -> TestHost {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let host = Self { dir, next_mod: AtomicU32::new(0) };
        if !config.is_empty() {
            std::fs::create_dir_all(host.config_dir()).unwrap_or_else(|error| panic!("config dir: {error}"));
            std::fs::write(host.config_dir().join("config.hcl"), config)
                .unwrap_or_else(|error| panic!("config file: {error}"));
        }
        let (config, game, version) =
            (host.config_dir().display().to_string(), host.game_dir().display().to_string(), "1.0.0-test");
        // SAFETY: the three texts live for the call
        let status = unsafe {
            amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
        };
        assert_eq!(status, AmmStatus::Ok);
        host
    }

    /// Runs `change` on the live runtime, as the panel does for a person's choice.
    pub fn with_runtime(&self, change: impl FnOnce(&mut HostRuntime)) {
        let result = HostCell::global().with(|runtime| {
            change(runtime);
            Ok(())
        });
        assert_eq!(result, Ok(()));
    }

    pub fn api() -> &'static AmmApi {
        let table = amm_api_for(AMM_ABI_MAJOR);
        // SAFETY: a non-null table is the manager's static table
        unsafe { table.as_ref() }.unwrap_or_else(|| panic!("table"))
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    /// A new absolute config folder for one test mod.
    pub fn next_mod_dir(&self) -> PathBuf {
        let n = self.next_mod.fetch_add(1, Ordering::SeqCst);
        self.dir.path().join("mods").join(format!("mod-{n}")).join("config")
    }

    pub fn game_dir(&self) -> PathBuf {
        self.dir.path().join("game")
    }

    pub fn frame(&self, now_ms: u64) -> AmmHostFrame {
        self.frame_with(now_ms, Self::PLAY)
    }

    /// One Present with `menu_screen` on top: `START`, `PAUSE` or `PLAY` (any other screen).
    pub fn frame_with(&self, now_ms: u64, menu_screen: u8) -> AmmHostFrame {
        self.present(now_ms, menu_screen, (Self::WIDTH, Self::HEIGHT), (Self::WIDTH, Self::HEIGHT))
    }

    /// One Present in play of a `back_buffer` sized back buffer in `R8G8B8A8_UNORM`, for a window whose client
    /// area is `client`.
    pub fn frame_sized(&self, now_ms: u64, back_buffer: (u32, u32), client: (u32, u32)) -> AmmHostFrame {
        self.present(now_ms, Self::PLAY, back_buffer, client)
    }

    fn present(&self, now_ms: u64, menu_screen: u8, back_buffer: (u32, u32), client: (u32, u32)) -> AmmHostFrame {
        let input = AmmHostFrameInput {
            width: back_buffer.0,
            height: back_buffer.1,
            pixels_per_point: 1.0,
            time_secs: now_ms as f64 / 1000.0,
            now_ms,
            menu_screen,
            format: Self::R8G8B8A8_UNORM,
            client_width: client.0,
            client_height: client.1,
        };
        let mut out = AmmHostFrame::empty();
        // SAFETY: input and out live for the call
        let status = unsafe { amm_host_frame(&input, &mut out) };
        assert_eq!(status, AmmStatus::Ok);
        out
    }

    pub fn layers(frame: &AmmHostFrame) -> &[AmmHostLayer] {
        if frame.layer_count == 0 {
            return &[];
        }
        // SAFETY: the host keeps layer_count layers at layers until the next frame
        unsafe { std::slice::from_raw_parts(frame.layers, frame.layer_count as usize) }
    }

    pub fn owners(frame: &AmmHostFrame) -> Vec<u8> {
        Self::layers(frame).iter().map(|layer| layer.owner).collect()
    }

    pub fn key(&self, vk: u32, down: bool, mods: u8, now_ms: u64) -> AmmHostRoute {
        amm_host_key(vk, down, false, mods, now_ms)
    }

    /// Where the manager's menu button is now in back-buffer pixels, or `None` while it is hidden.
    pub fn menu_button_rect() -> Option<[f32; 4]> {
        HostCell::global().with(|runtime| Ok(runtime.manager().menu_button_rect())).unwrap_or(None)
    }

    pub fn panel_open() -> bool {
        HostCell::global().with(|runtime| Ok(runtime.manager().snapshot().panel_open)).unwrap_or(false)
    }

    pub fn destroy(&self) {
        amm_host_destroy();
    }
}

impl Drop for TestHost {
    fn drop(&mut self) {
        amm_host_destroy();
    }
}
