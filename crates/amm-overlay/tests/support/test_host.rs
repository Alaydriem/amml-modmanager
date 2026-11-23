use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use amm_abi::types::AmmSamplerKind;
use amm_ffi::types::{AMM_HOST_GAME_PICTURE_TEXTURE, AmmHostFrame, AmmHostFrameInput, AmmHostLayer};
use amm_overlay::OverlayHost;

use super::{TestFrame, TestMesh};

/// An `OverlayHost` whose every frame is one full-screen red layer of a mod, made from a white texture it sends in
/// every frame. It counts what the overlay asks of it.
pub struct TestHost {
    ready: AtomicBool,
    /// `frame` gives `None`.
    no_frame: AtomicBool,
    /// `frame` gives the frame that draws the game picture over the whole target.
    picture: AtomicBool,
    /// `frame` gives a mod layer red over the left half and a manager layer blue over the right half.
    split: AtomicBool,
    /// `frame` gives a frame with no layer, as the manager's host does while nothing is shown.
    empty: AtomicBool,
    /// `frame` gives only the manager layer, blue over the right half.
    manager_only: AtomicBool,
    /// `frame` gives one mod layer whose mesh is empty: no geometry, no texture delta, no freed id.
    blank_layer: AtomicBool,
    frames: AtomicU32,
    textures_lost: AtomicU32,
    skipped: AtomicU32,
    plain: AmmHostFrame,
    with_picture: AmmHostFrame,
    with_split: AmmHostFrame,
    no_layers: AmmHostFrame,
    with_manager: AmmHostFrame,
    with_blank: AmmHostFrame,
    _frames: [TestFrame; 5],
    _meshes: [TestMesh; 5],
}

// SAFETY: the frames and the meshes they point into are never changed after `new`; the rest are atomics
unsafe impl Sync for TestHost {}

// SAFETY: the frames point into `_meshes` and `_frames`, which live as long as the host
unsafe impl OverlayHost for TestHost {
    fn game_ready(&self) -> bool {
        self.ready.load(Ordering::SeqCst)
    }

    fn frame(&self, _input: &AmmHostFrameInput) -> Option<&AmmHostFrame> {
        self.frames.fetch_add(1, Ordering::SeqCst);
        if self.no_frame.load(Ordering::SeqCst) {
            return None;
        }
        if self.empty.load(Ordering::SeqCst) {
            return Some(&self.no_layers);
        }
        if self.blank_layer.load(Ordering::SeqCst) {
            return Some(&self.with_blank);
        }
        if self.manager_only.load(Ordering::SeqCst) {
            return Some(&self.with_manager);
        }
        if self.split.load(Ordering::SeqCst) {
            return Some(&self.with_split);
        }
        if self.picture.load(Ordering::SeqCst) { Some(&self.with_picture) } else { Some(&self.plain) }
    }

    fn textures_lost(&self) {
        self.textures_lost.fetch_add(1, Ordering::SeqCst);
    }

    fn frame_skipped(&self) {
        self.skipped.fetch_add(1, Ordering::SeqCst);
    }
}

impl TestHost {
    pub const RED: [u8; 4] = [255, 0, 0, 255];
    pub const BLUE: [u8; 4] = [0, 0, 255, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];
    const LEFT: [f32; 4] = [0.0, 0.0, 32.0, 64.0];
    const RIGHT: [f32; 4] = [32.0, 0.0, 64.0, 64.0];
    const WHITE_TEXTURE: u64 = 1;
    const MOD_OWNER: u8 = 0;
    const SHADER_PICTURE: u8 = 2;
    const EVERYWHERE: [f32; 4] = [0.0, 0.0, 4096.0, 4096.0];

    /// A ready host that lives for the rest of the test process, as `SurfaceRenderer::new` needs.
    pub fn leaked() -> &'static TestHost {
        let host = Box::leak(Box::new(Self::new()));
        host.ready.store(true, Ordering::SeqCst);
        host
    }

    fn new() -> Self {
        let mut plain_mesh = TestMesh::default();
        plain_mesh.full(Self::WHITE_TEXTURE, 1, 1, Self::WHITE).quad(
            Self::EVERYWHERE,
            Self::RED,
            Self::WHITE_TEXTURE,
            Self::EVERYWHERE,
        );
        let mut picture_mesh = TestMesh::default();
        picture_mesh.quad_with(
            Self::EVERYWHERE,
            Self::WHITE,
            AMM_HOST_GAME_PICTURE_TEXTURE,
            Self::EVERYWHERE,
            Self::SHADER_PICTURE,
            AmmSamplerKind::Nearest as u8,
        );
        let mut left_mesh = TestMesh::default();
        left_mesh.full(Self::WHITE_TEXTURE, 1, 1, Self::WHITE).quad(
            Self::LEFT,
            Self::RED,
            Self::WHITE_TEXTURE,
            Self::EVERYWHERE,
        );
        let mut right_mesh = TestMesh::default();
        right_mesh.full(Self::WHITE_TEXTURE, 1, 1, Self::WHITE).quad(
            Self::RIGHT,
            Self::BLUE,
            Self::WHITE_TEXTURE,
            Self::EVERYWHERE,
        );
        let plain_frame = TestFrame::new(&[(Self::MOD_OWNER, &plain_mesh)]);
        let picture_frame = TestFrame::new(&[(Self::MOD_OWNER, &picture_mesh)]);
        let split_frame = TestFrame::new(&[(Self::MOD_OWNER, &left_mesh), (AmmHostLayer::MANAGER, &right_mesh)]);
        let manager_frame = TestFrame::new(&[(AmmHostLayer::MANAGER, &right_mesh)]);
        let blank_mesh = TestMesh::default();
        let blank_frame = TestFrame::new(&[(Self::MOD_OWNER, &blank_mesh)]);
        let plain = plain_frame.frame();
        let with_picture = AmmHostFrame { game_picture: true, ..picture_frame.frame() };
        let with_split = split_frame.frame();
        let with_manager = manager_frame.frame();
        let with_blank = blank_frame.frame();
        Self {
            ready: AtomicBool::new(false),
            no_frame: AtomicBool::new(false),
            picture: AtomicBool::new(false),
            split: AtomicBool::new(false),
            empty: AtomicBool::new(false),
            manager_only: AtomicBool::new(false),
            blank_layer: AtomicBool::new(false),
            frames: AtomicU32::new(0),
            textures_lost: AtomicU32::new(0),
            skipped: AtomicU32::new(0),
            plain,
            with_picture,
            with_split,
            no_layers: AmmHostFrame::empty(),
            with_manager,
            with_blank,
            _frames: [plain_frame, picture_frame, split_frame, manager_frame, blank_frame],
            _meshes: [plain_mesh, picture_mesh, left_mesh, right_mesh, blank_mesh],
        }
    }

    pub fn set_split(&self, split: bool) {
        self.split.store(split, Ordering::SeqCst);
    }

    pub fn set_empty(&self, empty: bool) {
        self.empty.store(empty, Ordering::SeqCst);
    }

    pub fn set_blank_layer(&self, blank: bool) {
        self.blank_layer.store(blank, Ordering::SeqCst);
    }

    pub fn set_manager_only(&self, manager_only: bool) {
        self.manager_only.store(manager_only, Ordering::SeqCst);
    }

    pub fn set_ready(&self, ready: bool) {
        self.ready.store(ready, Ordering::SeqCst);
    }

    pub fn set_no_frame(&self, none: bool) {
        self.no_frame.store(none, Ordering::SeqCst);
    }

    pub fn set_picture(&self, picture: bool) {
        self.picture.store(picture, Ordering::SeqCst);
    }

    /// How many times the overlay asked for a frame.
    pub fn frames(&self) -> u32 {
        self.frames.load(Ordering::SeqCst)
    }

    pub fn textures_lost(&self) -> u32 {
        self.textures_lost.load(Ordering::SeqCst)
    }

    pub fn skipped(&self) -> u32 {
        self.skipped.load(Ordering::SeqCst)
    }
}
