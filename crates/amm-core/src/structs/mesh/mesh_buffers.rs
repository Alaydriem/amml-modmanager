use super::{DrawCmd, TextureDelta, Vertex};

/// The output of one egui pass. The vectors keep their capacity across frames.
#[derive(Debug, Default)]
pub struct MeshBuffers {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub cmds: Vec<DrawCmd>,
    pub deltas: Vec<TextureDelta>,
    pub bytes: Vec<u8>,
    pub freed: Vec<u64>,
}

impl MeshBuffers {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.cmds.clear();
        self.deltas.clear();
        self.bytes.clear();
        self.freed.clear();
    }

    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }
}
