#![allow(unsafe_code)]

use std::ffi::c_void;
use std::sync::Arc;

use amm_abi::types::{AmmCallbacks, AmmDrawCmd, AmmFrameInput, AmmFrameOutput, AmmMeshView, AmmStatus, AmmVertex};

/// A client whose frame callback returns the same prebuilt mesh every time and runs no egui, so a measurement
/// of the host frame path is not mixed with a client's own work. The mesh is fixed after construction.
pub struct FakeMesh {
    vertices: Vec<AmmVertex>,
    indices: Vec<u32>,
    cmds: Vec<AmmDrawCmd>,
}

impl FakeMesh {
    const QUAD_VERTICES: usize = 4;
    const QUAD_INDICES: [u32; 6] = [0, 1, 2, 2, 1, 3];
    const SCREEN: [f32; 4] = [0.0, 0.0, 1920.0, 1080.0];

    /// A mesh of `vertex_count` vertices as quads (rounded down to a whole quad) with one draw command over all of
    /// its indices.
    pub fn quads(vertex_count: usize) -> Self {
        let quads = vertex_count / Self::QUAD_VERTICES;
        let mut vertices = Vec::with_capacity(quads * Self::QUAD_VERTICES);
        let mut indices = Vec::with_capacity(quads * Self::QUAD_INDICES.len());
        for quad in 0..quads {
            let (x, y) = ((quad % 64) as f32 * 8.0, (quad / 64) as f32 * 8.0);
            for (dx, dy) in [(0.0, 0.0), (8.0, 0.0), (0.0, 8.0), (8.0, 8.0)] {
                vertices.push(AmmVertex { pos: [x + dx, y + dy], uv: [0.0, 0.0], color: [255; 4] });
            }
            let base = (quad * Self::QUAD_VERTICES) as u32;
            indices.extend(Self::QUAD_INDICES.iter().map(|offset| base + offset));
        }
        let cmds = vec![AmmDrawCmd {
            texture: 0,
            index_offset: 0,
            index_count: indices.len() as u32,
            clip: Self::SCREEN,
            shader: 0,
            sampler: 0,
        }];
        Self { vertices, indices, cmds }
    }

    /// An empty mesh, for a client that only holds leases.
    pub fn empty() -> Self {
        Self { vertices: Vec::new(), indices: Vec::new(), cmds: Vec::new() }
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// The callbacks to register. The `Arc` must outlive every call the host makes.
    pub fn callbacks(mesh: &Arc<FakeMesh>) -> AmmCallbacks {
        AmmCallbacks {
            size: size_of::<AmmCallbacks>() as u32,
            user: Arc::as_ptr(mesh).cast_mut().cast::<c_void>(),
            frame: Some(Self::on_frame),
            textures_lost: Some(Self::on_textures_lost),
        }
    }

    fn view(&self) -> AmmMeshView {
        AmmMeshView {
            vertices: self.vertices.as_ptr(),
            vertex_count: self.vertices.len(),
            indices: self.indices.as_ptr(),
            index_count: self.indices.len(),
            cmds: self.cmds.as_ptr(),
            cmd_count: self.cmds.len(),
            deltas: std::ptr::null(),
            delta_count: 0,
            bytes: std::ptr::null(),
            byte_count: 0,
            freed: std::ptr::null(),
            freed_count: 0,
        }
    }

    unsafe extern "C" fn on_frame(user: *mut c_void, _input: *const AmmFrameInput, out: *mut AmmFrameOutput) -> i32 {
        // SAFETY: user is the FakeMesh of a live Arc that the bench host keeps, and out is valid for the call, as
        // the frame contract of the manager says
        let (mesh, out) = unsafe { (&*user.cast::<FakeMesh>(), &mut *out) };
        out.mesh = mesh.view();
        out.wants_pointer = 0;
        out.wants_keyboard = 0;
        out.cursor = 0;
        out.layer_rect = Self::SCREEN;
        AmmStatus::Ok as i32
    }

    unsafe extern "C" fn on_textures_lost(_user: *mut c_void) {}
}
