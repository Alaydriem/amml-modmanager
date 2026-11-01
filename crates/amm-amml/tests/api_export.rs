use amm_abi::types::{AMM_ABI_MAJOR, AMM_ABI_MINOR, AmmApi};
use amm_client::link::ApiTable;
use amm_core::errors::LinkError;
use windows::Win32::System::LibraryLoader::GetProcAddress;
use windows::core::s;

use super::support::manager_dll::ManagerDll;

type GetApi = unsafe extern "C" fn(abi_major: u32) -> *const AmmApi;

#[test]
fn a_client_finds_the_table_of_the_dll_loaded_under_the_manager_name() {
    assert!(matches!(ApiTable::resolve(), Err(LinkError::ManagerMissing)));
    let dll = ManagerDll::load_copy();

    assert!(ApiTable::resolve().is_ok());

    // SAFETY: the module is loaded and the name is NUL-terminated
    let proc = unsafe { GetProcAddress(dll.module, s!("amm_get_api")) }.unwrap();
    // SAFETY: the DLL exports amm_get_api with the GetApi signature
    let get_api = unsafe { std::mem::transmute::<unsafe extern "system" fn() -> isize, GetApi>(proc) };
    // SAFETY: amm_get_api takes a major version by value and returns null or the static table
    let table = unsafe { &*get_api(AMM_ABI_MAJOR) };
    assert_eq!((table.abi_major, table.abi_minor), (2, 1));
    assert_eq!(table.abi_minor, AMM_ABI_MINOR);
    // SAFETY: as above
    assert!(unsafe { get_api(AMM_ABI_MAJOR + 1) }.is_null());
    // a mod built for major 1 gets no table
    // SAFETY: as above
    assert!(unsafe { get_api(1) }.is_null());
}
