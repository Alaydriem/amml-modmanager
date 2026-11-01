use amm_abi::types::{AmmDrawCmd, AmmMeshView, AmmTextureDelta, AmmVertex};
use amm_ffi::types::{AmmHostFrame, AmmHostLayer};

/// The arrays of one `AmmMeshView` as slices. A null pointer or a count of 0 gives an empty slice.
pub(crate) struct MeshSlices<'a> {
    pub vertices: &'a [AmmVertex],
    pub indices: &'a [u32],
    pub cmds: &'a [AmmDrawCmd],
    pub deltas: &'a [AmmTextureDelta],
    pub bytes: &'a [u8],
    pub freed: &'a [u64],
}

impl<'a> MeshSlices<'a> {
    /// # Safety
    ///
    /// Each non-null pointer of `mesh` points to its count of live, aligned elements for `'a`.
    pub(crate) unsafe fn new(mesh: &'a AmmMeshView) -> Self {
        // SAFETY: each pointer and count pair comes from `mesh`, which the caller vouches for
        unsafe {
            Self {
                vertices: Self::slice(mesh.vertices, mesh.vertex_count),
                indices: Self::slice(mesh.indices, mesh.index_count),
                cmds: Self::slice(mesh.cmds, mesh.cmd_count),
                deltas: Self::slice(mesh.deltas, mesh.delta_count),
                bytes: Self::slice(mesh.bytes, mesh.byte_count),
                freed: Self::slice(mesh.freed, mesh.freed_count),
            }
        }
    }

    /// The layers of `frame`, or `None` when it has layers and a null pointer to them.
    ///
    /// # Safety
    ///
    /// A non-null `frame.layers` points to `frame.layer_count` live layers for `'a`.
    pub(crate) unsafe fn layers(frame: &'a AmmHostFrame) -> Option<&'a [AmmHostLayer]> {
        if frame.layer_count != 0 && frame.layers.is_null() {
            return None;
        }
        // SAFETY: the caller vouches for a non-null pointer and its count
        Some(unsafe { Self::slice(frame.layers, frame.layer_count as usize) })
    }

    /// A mesh with vertices, indices and commands. A mesh without them is not drawn and takes no buffer space.
    pub(crate) fn usable(&self) -> bool {
        !self.vertices.is_empty() && !self.indices.is_empty() && !self.cmds.is_empty()
    }

    unsafe fn slice<T>(ptr: *const T, len: usize) -> &'a [T] {
        if ptr.is_null() || len == 0 {
            return &[];
        }
        // SAFETY: non-null, and the caller vouches for `len` live, aligned elements for 'a
        unsafe { std::slice::from_raw_parts(ptr, len) }
    }
}
