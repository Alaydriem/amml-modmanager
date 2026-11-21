use super::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmAcquireResult, AmmHandle};

/// The registry table `alaydriem.modmanager.leases`: the manager's capability leases, `Camera`, `Hud`, `Overlay`,
/// `WorldDraw` and `RenderSize` (`AmmCapability`). Game input is not a manager lease: a mod takes it from the loader.
/// Game thread unless a function says otherwise. New functions are only added at the end, with a higher minor. A null
/// slot means the function is absent. After the manager is disabled `acquire` is denied with no holder and the others
/// answer 0 or nothing.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmLeasesTable {
    pub size: u32,
    pub version: u32,
    /// Asks for a capability lease. `capability` is an `AmmCapability` value; any other value, such as the
    /// unused 1, is answered as denied with no holder.
    pub acquire: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8) -> AmmAcquireResult>,
    /// Gives a lease back. `capability` is an `AmmCapability` value; any other value is ignored.
    pub release: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8)>,
    /// Any thread. The id of the mod that holds the lease now, copied as UTF-8 into `out_buf` (room for `out_cap`
    /// bytes); returns its length. 0 means no mod holds it, or `capability` is not an `AmmCapability` value. A return
    /// value greater than `out_cap` means the buffer was too small and nothing was written. `out_buf` may be null
    /// when `out_cap` is 0.
    pub holder: Option<unsafe extern "C" fn(capability: u8, out_buf: *mut u8, out_cap: usize) -> u32>,
    /// 1 when a mod other than the caller holds `capability` now, else 0. A bad handle or a value that is not an
    /// `AmmCapability` gives 0, as does a call from a frame callback (the host is busy composing the frame).
    pub held_by_other: Option<unsafe extern "C" fn(handle: AmmHandle, capability: u8) -> u8>,
}

// SAFETY: `AmmLeasesTable` is `#[repr(C)]` and starts with `size: u32, version: u32`, and `size` is the byte size of
// the provider's table.
#[allow(unsafe_code)]
unsafe impl amml_api::Provided for AmmLeasesTable {
    const NAME: &'static str = "alaydriem.modmanager.leases";
    const MAJOR: u16 = AMM_ABI_MAJOR as u16;
    const MINOR: u16 = AMM_ABI_MINOR as u16;
}
