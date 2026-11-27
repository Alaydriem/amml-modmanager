use std::ffi::c_void;
use std::sync::Mutex;

use amm_abi::types::{
    AmmCallbacks, AmmCapability, AmmDrawCmd, AmmFrameInput, AmmFrameOutput, AmmHandle, AmmMeshView, AmmStatus,
    AmmVertex,
};

use super::{Descriptors, TestClient, TestHost, TestMesh};

/// A mod with a Hud and an Overlay layer that fills one shared vertex buffer for both, as the ABI allows: the mesh
/// of a frame call is valid only until the mod's next frame call. Each call moves the buffer to a new allocation
/// and poisons the old one (all colors 0), keeping it alive so a late read of it is defined and visible.
pub struct SharedMeshClient {
    // kept alive for the callbacks, which get a pointer to it
    _state: Box<SharedMeshState>,
    handle: Option<AmmHandle>,
}

struct SharedMeshState {
    current: Mutex<Vec<AmmVertex>>,
    graveyard: Mutex<Vec<Vec<AmmVertex>>>,
    indices: Vec<u32>,
    cmds: Vec<AmmDrawCmd>,
}

impl SharedMeshClient {
    pub const HUD_COLOR: [u8; 4] = [255, 0, 0, 255];
    pub const OVERLAY_COLOR: [u8; 4] = [0, 0, 255, 255];
    pub const HUD_VERTICES: usize = 3;
    pub const OVERLAY_VERTICES: usize = 6;

    pub fn register(host: &TestHost, id: &str) -> SharedMeshClient {
        let state = Box::new(SharedMeshState {
            current: Mutex::new(Vec::new()),
            graveyard: Mutex::new(Vec::new()),
            indices: vec![0, 1, 2],
            cmds: vec![TestMesh::cmd([0.0, 0.0, 100.0, 100.0])],
        });
        let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
        let handle = TestClient::register_handle(&Descriptors::with_claims(id, &claims, ""), &host.next_mod_dir())
            .unwrap_or_else(|status| panic!("register: {status:?}"));
        let callbacks = AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: std::ptr::from_ref::<SharedMeshState>(&state).cast_mut().cast::<c_void>(),
            frame: Some(Self::on_frame),
            textures_lost: None,
        };
        let set_callbacks = TestHost::layers_table().set_callbacks.unwrap_or_else(|| panic!("set_callbacks"));
        // SAFETY: the callbacks live for the call; the state lives as long as the client
        assert_eq!(unsafe { set_callbacks(handle, &callbacks) }, AmmStatus::Ok);
        let acquire = TestHost::leases_table().acquire.unwrap_or_else(|| panic!("acquire"));
        for capability in [AmmCapability::Hud, AmmCapability::Overlay] {
            // SAFETY: acquire takes plain values
            unsafe { acquire(handle, capability as u8) };
        }
        Self { _state: state, handle: Some(handle) }
    }

    pub fn owner(&self) -> u8 {
        self.handle.map_or(u8::MAX, |handle| handle.slot as u8)
    }

    unsafe extern "C" fn on_frame(user: *mut c_void, input: *const AmmFrameInput, out: *mut AmmFrameOutput) -> i32 {
        // SAFETY: user is the state of a live client, and input and out are valid for the call
        let (state, input, out) = unsafe { (&*user.cast::<SharedMeshState>(), &*input, &mut *out) };
        let (color, count) = if input.allowed_layers == 1 {
            (Self::HUD_COLOR, Self::HUD_VERTICES)
        } else {
            (Self::OVERLAY_COLOR, Self::OVERLAY_VERTICES)
        };
        let mut current = state.current.lock().unwrap_or_else(|error| panic!("current: {error}"));
        let mut old = std::mem::replace(&mut *current, vec![AmmVertex { color, ..TestMesh::VERTEX }; count]);
        old.iter_mut().for_each(|vertex| vertex.color = [0; 4]);
        state.graveyard.lock().unwrap_or_else(|error| panic!("graveyard: {error}")).push(old);
        out.mesh = AmmMeshView {
            vertices: current.as_ptr(),
            vertex_count: current.len(),
            indices: state.indices.as_ptr(),
            index_count: state.indices.len(),
            cmds: state.cmds.as_ptr(),
            cmd_count: state.cmds.len(),
            deltas: std::ptr::null(),
            delta_count: 0,
            bytes: std::ptr::null(),
            byte_count: 0,
            freed: std::ptr::null(),
            freed_count: 0,
        };
        out.layer_rect = [0.0, 0.0, 100.0, 100.0];
        AmmStatus::Ok as i32
    }
}

impl Drop for SharedMeshClient {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            TestClient::unregister_handle(handle);
        }
    }
}
