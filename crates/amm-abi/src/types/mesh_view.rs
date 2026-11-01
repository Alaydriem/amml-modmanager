use super::{AmmDrawCmd, AmmTextureDelta, AmmVertex};

/// A borrowed view of one egui pass. Every pointer stays valid until the next call of the same callback.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmMeshView {
    pub vertices: *const AmmVertex,
    pub vertex_count: usize,
    pub indices: *const u32,
    pub index_count: usize,
    pub cmds: *const AmmDrawCmd,
    pub cmd_count: usize,
    pub deltas: *const AmmTextureDelta,
    pub delta_count: usize,
    pub bytes: *const u8,
    pub byte_count: usize,
    pub freed: *const u64,
    pub freed_count: usize,
}

impl AmmMeshView {
    /// A view of no vertices, indices, commands, texture changes or bytes.
    pub fn empty() -> Self {
        Self {
            vertices: std::ptr::null(),
            vertex_count: 0,
            indices: std::ptr::null(),
            index_count: 0,
            cmds: std::ptr::null(),
            cmd_count: 0,
            deltas: std::ptr::null(),
            delta_count: 0,
            bytes: std::ptr::null(),
            byte_count: 0,
            freed: std::ptr::null(),
            freed_count: 0,
        }
    }
}
