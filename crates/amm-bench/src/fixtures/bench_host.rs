#![allow(unsafe_code)]

use std::path::PathBuf;
use std::sync::Arc;

use amm_abi::types::{AmmCallbacks, AmmHandle, AmmStatus};
use amm_core::structs::descriptor::Capability;
use amm_core::structs::render::RenderSize;
use amm_ffi::host::{amm_host_create, amm_host_destroy, amm_host_frame};
use amm_ffi::services::HostCell;
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput};
use tempfile::TempDir;

use super::{Descriptors, FakeMesh};

/// The process-global host in a temporary folder, with fake mesh clients, destroyed on drop.
pub struct BenchHost {
    dir: TempDir,
    clients: Vec<Arc<FakeMesh>>,
}

impl BenchHost {
    pub const WIDTH: u32 = 1920;
    pub const HEIGHT: u32 = 1080;

    pub fn new() -> Self {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error}"));
        let host = Self { dir, clients: Vec::new() };
        let config = host.dir.path().join("config").display().to_string();
        let game = host.game_dir().display().to_string();
        let version = "1.0.0-bench";
        // SAFETY: the three texts live for the call
        let status = unsafe {
            amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
        };
        assert_eq!(status, AmmStatus::Ok);
        host
    }

    /// The host with `count` clients, each drawing `vertices` vertices on an Overlay lease.
    pub fn with_overlay_clients(count: usize, vertices: usize) -> Self {
        let mut host = Self::new();
        for n in 0..count {
            let descriptor = Descriptors::with_claims(&format!("client{n}"), &[("Overlay", "Shared")]);
            let handle = host.add_client(&descriptor, FakeMesh::quads(vertices));
            host.acquire(handle, Capability::Overlay);
        }
        host
    }

    /// The host with `count` clients, each drawing `vertices` vertices on a Hud lease and on an Overlay lease, so
    /// each client has two layers.
    pub fn with_two_layer_clients(count: usize, vertices: usize) -> Self {
        let mut host = Self::new();
        for n in 0..count {
            let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
            let descriptor = Descriptors::with_claims(&format!("client{n}"), &claims);
            let handle = host.add_client(&descriptor, FakeMesh::quads(vertices));
            host.acquire(handle, Capability::Hud);
            host.acquire(handle, Capability::Overlay);
        }
        host
    }

    /// The host with one client that registers without a frame callback and holds a WorldDraw lease, so it has no
    /// layer.
    pub fn with_world_draw_client_without_frame() -> Self {
        let mut host = Self::new();
        let descriptor = Descriptors::with_claims("worldly", &[("WorldDraw", "Shared")]);
        let callbacks = AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: std::ptr::null_mut(),
            frame: None,
            textures_lost: None,
        };
        let handle = host.add_client_with(&descriptor, callbacks, Arc::new(FakeMesh::empty()));
        host.acquire(handle, Capability::WorldDraw);
        host
    }

    /// The host with two clients that claim Camera exclusively and the second one denied, so one toast is queued.
    pub fn with_denied_camera() -> Self {
        let mut host = Self::new();
        for n in 0..2 {
            let descriptor = Descriptors::with_claims(&format!("camera{n}"), &[("Camera", "Exclusive")]);
            let handle = host.add_client(&descriptor, FakeMesh::empty());
            host.acquire(handle, Capability::Camera);
        }
        host
    }

    /// The host with one client that holds `RenderSize` and forces `width` x `height`, so every Present takes the
    /// locked path of `HostCell::frame` to reach the render size book.
    pub fn with_forced_size(width: u32, height: u32) -> Self {
        let mut host = Self::new();
        let descriptor = Descriptors::with_claims("sizer", &[("RenderSize", "Exclusive")]);
        let handle = host.add_client(&descriptor, FakeMesh::empty());
        host.acquire(handle, Capability::RenderSize);
        let forced =
            HostCell::global().with(|runtime| runtime.set_render_size(handle, Some(RenderSize::new(width, height))));
        forced.unwrap_or_else(|status| panic!("set_render_size: {status:?}"));
        host
    }

    /// Registers a client with the fake mesh as its frame output.
    pub fn add_client(&mut self, descriptor: &str, mesh: FakeMesh) -> AmmHandle {
        let mesh = Arc::new(mesh);
        let callbacks = FakeMesh::callbacks(&mesh);
        self.add_client_with(descriptor, callbacks, mesh)
    }

    fn add_client_with(&mut self, descriptor: &str, callbacks: AmmCallbacks, mesh: Arc<FakeMesh>) -> AmmHandle {
        let dir = self.dir.path().join("mods").join(format!("mod-{}", self.clients.len())).join("config");
        let registered = HostCell::global().with(|runtime| runtime.register(descriptor.as_bytes(), &dir, callbacks));
        self.clients.push(mesh);
        registered.unwrap_or_else(|status| panic!("register: {status:?}"))
    }

    pub fn acquire(&self, handle: AmmHandle, capability: Capability) {
        let acquired = HostCell::global().with(|runtime| runtime.acquire(handle, capability));
        acquired.unwrap_or_else(|status| panic!("acquire: {status:?}"));
    }

    /// The vertices the clients return in one frame.
    pub fn vertices_per_frame(&self) -> usize {
        self.clients.iter().map(|client| client.vertex_count()).sum()
    }

    /// One Present through the exported entry point.
    pub fn frame(&self, now_ms: u64) -> AmmHostFrame {
        self.frame_at(now_ms, Self::WIDTH, Self::HEIGHT)
    }

    /// One Present of a `width` x `height` back buffer in a client area of the same size.
    pub fn frame_at(&self, now_ms: u64, width: u32, height: u32) -> AmmHostFrame {
        let input = AmmHostFrameInput {
            width,
            height,
            pixels_per_point: 1.0,
            time_secs: now_ms as f64 / 1000.0,
            now_ms,
            menu_screen: 0,
            format: 28,
            client_width: width,
            client_height: height,
        };
        let mut out = AmmHostFrame::empty();
        // SAFETY: input and out live for the call
        let status = unsafe { amm_host_frame(&input, &mut out) };
        assert_eq!(status, AmmStatus::Ok);
        out
    }

    pub fn game_dir(&self) -> PathBuf {
        self.dir.path().join("game")
    }
}

impl Default for BenchHost {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for BenchHost {
    fn drop(&mut self) {
        amm_host_destroy();
    }
}
