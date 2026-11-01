use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_abi::types::{AMM_ABI_MAJOR, AmmLogLevel, AmmStatus};
use amm_client::link::ApiTable;
use amm_core::structs::toast::ToastKind;
use amm_ffi::api::amm_api_for;
use amm_ffi::host::{amm_host_create, amm_host_destroy, amm_host_frame, amm_host_key, amm_host_log, amm_host_tick};
use amm_ffi::services::{HostCell, HostRuntime};
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput, AmmHostLayer, AmmHostRoute};
use tempfile::TempDir;

/// The process-global host in temporary config and game folders, destroyed on drop.
pub struct HostFixture {
    dir: TempDir,
    next_mod: AtomicU32,
}

impl HostFixture {
    pub const WIDTH: u32 = 1920;
    pub const HEIGHT: u32 = 1080;
    pub const MANAGER: u8 = 0xFE;
    pub const CTRL: u8 = 1;

    pub fn new() -> HostFixture {
        Self::with_config("")
    }

    /// Starts the host with `config_hcl` as the manager's own `config.hcl`, unless it is empty.
    pub fn with_config(config_hcl: &str) -> HostFixture {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let fixture = Self { dir, next_mod: AtomicU32::new(0) };
        fs::create_dir_all(fixture.config_dir()).unwrap_or_else(|error| panic!("config dir: {error}"));
        if !config_hcl.is_empty() {
            fs::write(fixture.config_dir().join("config.hcl"), config_hcl)
                .unwrap_or_else(|error| panic!("manager config: {error}"));
        }
        let (config, game, version) =
            (fixture.config_dir().display().to_string(), fixture.game_dir().display().to_string(), "1.0.0-e2e");
        // SAFETY: the three texts live for the call
        let status = unsafe {
            amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
        };
        assert_eq!(status, AmmStatus::Ok);
        fixture
    }

    pub fn table(&self) -> ApiTable {
        let ptr = amm_api_for(AMM_ABI_MAJOR);
        // SAFETY: amm_api_for returns null or the manager's static table
        unsafe { ApiTable::new(ptr) }.unwrap_or_else(|error| panic!("api table: {error:?}"))
    }

    pub fn config_dir(&self) -> PathBuf {
        self.dir.path().join("config")
    }

    pub fn game_dir(&self) -> PathBuf {
        self.dir.path().join("game")
    }

    /// The folder holding the session logs of the mod with this id.
    pub fn log_dir(&self, namespace: &str, name: &str) -> PathBuf {
        self.game_dir().join("logs").join("amm").join(format!("@{namespace}")).join(name)
    }

    /// A new absolute config folder for one simulated mod.
    pub fn next_mod_dir(&self) -> PathBuf {
        self.mod_dir(self.next_mod.fetch_add(1, Ordering::SeqCst))
    }

    /// The config folder `next_mod_dir` gave the `n`th simulated mod.
    pub fn mod_dir(&self, n: u32) -> PathBuf {
        self.dir.path().join("mods").join(format!("mod-{n}")).join("config")
    }

    pub fn key(&self, vk: u32, down: bool, mods: u8, now_ms: u64) -> AmmHostRoute {
        amm_host_key(vk, down, false, mods, now_ms)
    }

    pub fn tick(&self, now_ms: u64) {
        amm_host_tick(now_ms);
    }

    pub fn frame(&self, now_ms: u64) -> AmmHostFrame {
        let input = AmmHostFrameInput {
            width: Self::WIDTH,
            height: Self::HEIGHT,
            pixels_per_point: 1.0,
            time_secs: now_ms as f64 / 1000.0,
            now_ms,
            menu_screen: 0,
            format: 28,
            client_width: Self::WIDTH,
            client_height: Self::HEIGHT,
        };
        let mut out = AmmHostFrame::empty();
        // SAFETY: input and out live for the call
        let status = unsafe { amm_host_frame(&input, &mut out) };
        assert_eq!(status, AmmStatus::Ok);
        out
    }

    pub fn owners(frame: &AmmHostFrame) -> Vec<u8> {
        if frame.layer_count == 0 {
            return Vec::new();
        }
        // SAFETY: the host keeps layer_count layers at layers until the next frame
        let layers: &[AmmHostLayer] = unsafe { std::slice::from_raw_parts(frame.layers, frame.layer_count as usize) };
        layers.iter().map(|layer| layer.owner).collect()
    }

    /// Runs `f` on the host runtime under its lock.
    pub fn with_runtime<T>(&self, f: impl FnOnce(&mut HostRuntime) -> T) -> T {
        HostCell::global().with(|runtime| Ok(f(runtime))).unwrap_or_else(|error| panic!("host runtime: {error:?}"))
    }

    /// The kinds of every toast the manager queued so far, oldest first.
    pub fn toasts(&self) -> Vec<ToastKind> {
        self.with_runtime(|runtime| runtime.manager().toasts().visible(0, u64::MAX).map(|toast| toast.kind).collect())
    }

    /// One log line of the manager mod, as `amm_host_log` takes it.
    pub fn host_log(&self, level: AmmLogLevel, message: &str) {
        let target = "e2e::host";
        // SAFETY: both texts live for the call, and the null fields pointers come with zero lengths
        unsafe {
            amm_host_log(
                level as u8,
                target.as_ptr(),
                target.len(),
                message.as_ptr(),
                message.len(),
                std::ptr::null(),
                0,
                std::ptr::null(),
                0,
                0,
            );
        }
    }

    /// Stops the host, which flushes the config writer and closes the log files. Safe to call twice.
    pub fn destroy(&self) {
        amm_host_destroy();
    }
}

impl Drop for HostFixture {
    fn drop(&mut self) {
        amm_host_destroy();
    }
}
