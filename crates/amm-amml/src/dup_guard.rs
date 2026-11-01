use windows::Win32::Foundation::HMODULE;
use windows::Win32::System::LibraryLoader::{
    GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, GetModuleHandleExW,
    GetModuleHandleW,
};
use windows::core::PCWSTR;

use crate::errors::DupError;
use crate::module_path::ModulePath;

/// Mods find the manager by its module name, so only one module of that name may be the manager. The C++
/// manager of LeviLamina has another name but answers the same lookups; when it, or any other copy, is loaded
/// first, this copy stays off.
pub struct DupGuard;

impl DupGuard {
    pub const MODULE_NAME: &'static str = "amml_modmanager.dll";
    /// The module name of the C++ manager.
    pub const CPP_MANAGER: &'static str = "alaydriem-mod-manager.dll";

    /// Ok when the module mods find by `MODULE_NAME` is `own` and no module named `CPP_MANAGER` is loaded. A
    /// loaded C++ manager, or another module named `MODULE_NAME`, gives its path; no module named `MODULE_NAME`
    /// means `own` was loaded under another name.
    pub fn check(own: HMODULE) -> Result<(), DupError> {
        if let Some(module) = Self::loaded(Self::CPP_MANAGER) {
            return Err(DupError::OtherCopy { path: ModulePath::of(Some(module)).unwrap_or_default() });
        }
        match Self::loaded(Self::MODULE_NAME) {
            Some(module) if module == own => Ok(()),
            Some(module) => Err(DupError::OtherCopy { path: ModulePath::of(Some(module)).unwrap_or_default() }),
            None => Err(DupError::NotFound),
        }
    }

    // The loaded module with this file name, if any. The lookup adds no reference.
    fn loaded(file_name: &str) -> Option<HMODULE> {
        let name: Vec<u16> = file_name.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: the name is NUL-terminated UTF-16 and lives for the call
        #[allow(unsafe_code)]
        let found = unsafe { GetModuleHandleW(PCWSTR(name.as_ptr())) };
        found.ok()
    }

    /// The module this code is in.
    pub fn own_module() -> Result<HMODULE, DupError> {
        let mut module = HMODULE::default();
        let address = Self::own_module as *const () as *const u16;
        // SAFETY: with FROM_ADDRESS the name argument is an address inside this module, and `module` is writable
        #[allow(unsafe_code)]
        let found = unsafe {
            GetModuleHandleExW(
                GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT,
                PCWSTR(address),
                &mut module,
            )
        };
        found.map(|()| module).map_err(|_| DupError::OwnModule)
    }
}
