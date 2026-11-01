use amm_abi::types::{AmmDrawCmd, AmmMeshView, AmmTextureDelta, AmmVertex};

/// The mesh a test client returns from each frame. The vectors are fixed after construction, so the view stays
/// valid for the life of the client.
pub struct TestMesh {
    pub vertices: Vec<AmmVertex>,
    pub indices: Vec<u32>,
    pub cmds: Vec<AmmDrawCmd>,
    pub deltas: Vec<AmmTextureDelta>,
    pub bytes: Vec<u8>,
    pub freed: Vec<u64>,
}

impl TestMesh {
    pub const VERTEX: AmmVertex = AmmVertex { pos: [0.0, 0.0], uv: [0.0, 0.0], color: [255; 4] };

    /// One triangle with one draw command over the whole screen.
    pub fn triangle() -> Self {
        Self::with_cmds(vec![Self::cmd([0.0, 0.0, 100.0, 100.0])])
    }

    /// Three vertices, three indices and the given draw commands.
    pub fn with_cmds(cmds: Vec<AmmDrawCmd>) -> Self {
        Self {
            vertices: vec![Self::VERTEX; 3],
            indices: vec![0, 1, 2],
            cmds,
            deltas: Vec::new(),
            bytes: Vec::new(),
            freed: Vec::new(),
        }
    }

    /// `count` vertices and one triangle.
    pub fn with_vertices(count: usize) -> Self {
        Self { vertices: vec![Self::VERTEX; count], ..Self::triangle() }
    }

    /// A triangle plus one full upload of a `side` x `side` texture with id `id`.
    pub fn with_texture(id: u64, side: u32) -> Self {
        let byte_len = side * side * 4;
        let delta = AmmTextureDelta {
            id,
            sampler: 0,
            full: 1,
            x: 0,
            y: 0,
            width: side,
            height: side,
            byte_offset: 0,
            byte_len,
        };
        Self { deltas: vec![delta], bytes: vec![0; byte_len as usize], ..Self::triangle() }
    }

    pub fn cmd(clip: [f32; 4]) -> AmmDrawCmd {
        AmmDrawCmd { texture: 0, index_offset: 0, index_count: 3, clip, shader: 0, sampler: 0 }
    }

    pub fn view(&self) -> AmmMeshView {
        AmmMeshView {
            vertices: self.vertices.as_ptr(),
            vertex_count: self.vertices.len(),
            indices: self.indices.as_ptr(),
            index_count: self.indices.len(),
            cmds: self.cmds.as_ptr(),
            cmd_count: self.cmds.len(),
            deltas: self.deltas.as_ptr(),
            delta_count: self.deltas.len(),
            bytes: self.bytes.as_ptr(),
            byte_count: self.bytes.len(),
            freed: self.freed.as_ptr(),
            freed_count: self.freed.len(),
        }
    }
}
