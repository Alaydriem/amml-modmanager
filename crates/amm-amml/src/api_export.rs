use amm_abi::types::AmmApi;

/// The manager's function table for `abi_major`, or null for a major version this manager does not speak. Mods
/// find it with `GetModuleHandleW("amml_modmanager.dll")` and `GetProcAddress`. The table is static and
/// lives as long as this DLL.
#[allow(unsafe_code)]
#[unsafe(no_mangle)]
pub extern "C" fn amm_get_api(abi_major: u32) -> *const AmmApi {
    amm_ffi::api::amm_api_for(abi_major)
}
