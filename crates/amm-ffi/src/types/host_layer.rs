use amm_abi::types::AmmMeshView;

/// One mesh to draw. `owner` is the mod slot whose texture ids the mesh uses, or `MANAGER` for the manager's own
/// panel and toasts. Every clip rect in `mesh.cmds` is inside the back buffer and not empty.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmHostLayer {
    pub owner: u8,
    pub mesh: AmmMeshView,
}

impl AmmHostLayer {
    /// The owner of the manager's own layer.
    pub const MANAGER: u8 = 0xFE;
}
