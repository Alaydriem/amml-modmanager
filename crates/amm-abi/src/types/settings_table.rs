use super::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmHandle, AmmStatus};

/// The registry table `alaydriem.modmanager.settings`: the mod's saved settings. Game thread. New functions are only
/// added at the end, with a higher minor. A null slot means the function is absent. After the manager is disabled
/// every function answers `Disabled`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct AmmSettingsTable {
    pub size: u32,
    pub version: u32,
    /// The checked `config.hcl` text of the mod. The text at `out_ptr` is owned by the manager and is valid until
    /// the next `config_document` call for this handle or a `SettingsChanged` event.
    pub config_document:
        Option<unsafe extern "C" fn(handle: AmmHandle, out_ptr: *mut *const u8, out_len: *mut usize) -> AmmStatus>,
    /// A mod changes one of its own saved values with one HCL expression. `path` and `value_hcl` are valid only for
    /// the call. The mod that made the change gets no `SettingsChanged`.
    pub set_setting: Option<
        unsafe extern "C" fn(
            handle: AmmHandle,
            path: *const u8,
            path_len: usize,
            value_hcl: *const u8,
            value_len: usize,
        ) -> AmmStatus,
    >,
}

// SAFETY: `AmmSettingsTable` is `#[repr(C)]` and starts with `size: u32, version: u32`, and `size` is the byte size
// of the provider's table.
#[allow(unsafe_code)]
unsafe impl amml_api::Provided for AmmSettingsTable {
    const NAME: &'static str = "alaydriem.modmanager.settings";
    const MAJOR: u16 = AMM_ABI_MAJOR as u16;
    const MINOR: u16 = AMM_ABI_MINOR as u16;
}
