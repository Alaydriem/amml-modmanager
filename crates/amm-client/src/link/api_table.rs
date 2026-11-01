use amm_abi::types::{AMM_ABI_MAJOR, AmmApi};
use amm_core::errors::LinkError;
use amm_core::services::abi::AbiPrefix;
use amm_core::traits::AbiSized;
use std::mem::MaybeUninit;
use windows_sys::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};

/// The manager's function table after its version and size are checked. The table is copied once. A function the
/// manager's table is too short to hold is a null slot in the copy, so a client that calls a function added in a
/// later minor version gets `None` from an older manager and never reads past the table.
#[derive(Debug, Clone, Copy)]
pub struct ApiTable {
    api: AmmApi,
}

type GetApi = unsafe extern "C" fn(abi_major: u32) -> *const AmmApi;

impl ApiTable {
    const MANAGER_DLL: &'static str = "amml_modmanager.dll";
    const GET_API: &'static [u8] = b"amm_get_api\0";

    /// Checks the table at `ptr`: null is `ManagerMissing`; another major version, or a table smaller than the
    /// functions of minor version 0, is `AbiMismatch`. Any minor version is accepted. The copy keeps the bytes
    /// the table declares in `size`, up to the size of this client's `AmmApi`; every later slot is null.
    ///
    /// # Safety
    ///
    /// `ptr` is null or points at a table that starts with the `size`, `abi_major` and `abi_minor` header and
    /// holds at least `size` readable bytes for the call.
    pub unsafe fn new(ptr: *const AmmApi) -> Result<ApiTable, LinkError> {
        if ptr.is_null() {
            return Err(LinkError::ManagerMissing);
        }
        // SAFETY: the caller guarantees a non-null table starts with the header, so these two reads are in bounds
        let (size, abi_major) = unsafe { ((&raw const (*ptr).size).read(), (&raw const (*ptr).abi_major).read()) };
        if abi_major != AMM_ABI_MAJOR {
            return Err(LinkError::AbiMismatch { found: abi_major });
        }
        let Some(copy_len) = AbiPrefix::len(size, AmmApi::BASE_SIZE, size_of::<AmmApi>()) else {
            return Err(LinkError::AbiMismatch { found: abi_major });
        };
        let mut api = MaybeUninit::<AmmApi>::zeroed();
        // SAFETY: the caller guarantees `size` readable bytes at ptr and copy_len <= size; the destination holds
        // size_of::<AmmApi>() >= copy_len bytes and does not overlap the manager's table
        unsafe { std::ptr::copy_nonoverlapping(ptr.cast::<u8>(), api.as_mut_ptr().cast::<u8>(), copy_len) };
        // SAFETY: every field of AmmApi is an integer or an Option of a function pointer, and all-zero bytes
        // plus the copied table bytes are a valid value of each
        let api = unsafe { api.assume_init() };
        Ok(Self { api })
    }

    /// Finds the loaded manager DLL and asks it for the table of this client's major version.
    pub fn resolve() -> Result<ApiTable, LinkError> {
        let name: Vec<u16> = Self::MANAGER_DLL.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: name is a NUL-terminated UTF-16 string that lives for the call
        let module = unsafe { GetModuleHandleW(name.as_ptr()) };
        if module.is_null() {
            return Err(LinkError::ManagerMissing);
        }
        // SAFETY: module is a loaded module handle and GET_API is a NUL-terminated ASCII name
        let Some(proc) = (unsafe { GetProcAddress(module, Self::GET_API.as_ptr()) }) else {
            return Err(LinkError::ManagerMissing);
        };
        // SAFETY: the manager exports amm_get_api with the GetApi signature
        let get_api = unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, GetApi>(proc) };
        // SAFETY: amm_get_api takes a major version by value and returns null or the manager's static table
        let ptr = unsafe { get_api(AMM_ABI_MAJOR) };
        // SAFETY: a non-null result is the manager's static table, which lives as long as the manager DLL
        unsafe { Self::new(ptr) }
    }

    pub(crate) fn api(&self) -> &AmmApi {
        &self.api
    }
}
