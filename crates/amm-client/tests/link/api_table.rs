use amm_abi::types::{AMM_ABI_MAJOR, AmmApi};
use std::path::Path;

use amm_client::link::{ApiTable, ManagerLink};
use amm_core::errors::LinkError;
use amm_core::traits::AbiSized;

use crate::support::{FakeTable, IdleHandler};

#[test]
fn wrong_abi_major_is_abi_mismatch() {
    let api = FakeTable::leak(AmmApi { abi_major: AMM_ABI_MAJOR + 1, ..FakeTable::api() });

    // SAFETY: the table is leaked, so it stays valid for the process
    let result = unsafe { ApiTable::new(api) };

    assert_eq!(result.err(), Some(LinkError::AbiMismatch { found: AMM_ABI_MAJOR + 1 }));
}

#[test]
fn table_smaller_than_this_client_needs_is_abi_mismatch() {
    let base = std::mem::offset_of!(AmmApi, log) as u32;
    let api = FakeTable::leak(AmmApi { size: base, ..FakeTable::api() });

    // SAFETY: the table is leaked, so it stays valid for the process
    let result = unsafe { ApiTable::new(api) };

    assert_eq!(result.err(), Some(LinkError::AbiMismatch { found: AMM_ABI_MAJOR }));
}

#[test]
fn null_table_is_manager_missing() {
    // SAFETY: a null pointer is never read
    let result = unsafe { ApiTable::new(std::ptr::null()) };

    assert_eq!(result.err(), Some(LinkError::ManagerMissing));
}

#[repr(C)]
struct FutureTable {
    api: AmmApi,
    added_later: [usize; 4],
}

#[test]
fn a_client_accepts_a_newer_minor_table_and_links_through_it() {
    let future = Box::leak(Box::new(FutureTable {
        api: AmmApi { size: size_of::<FutureTable>() as u32, abi_minor: 10, ..FakeTable::api() },
        added_later: [usize::MAX; 4],
    }));

    // SAFETY: the table is leaked and its first field is an AmmApi header that declares the whole struct
    let table = unsafe { ApiTable::new(std::ptr::from_ref(&future.api)) }.expect("newer minor accepted");
    let link = ManagerLink::new(
        table,
        "namespace = \"test\"\nname = \"alaydriem-test\"\n",
        Path::new(FakeTable::CONFIG_DIR),
        IdleHandler,
    );

    assert!(link.is_ok());
}

#[test]
fn a_client_accepts_a_table_that_declares_exactly_the_functions_of_2_0() {
    let api = FakeTable::leak(AmmApi { size: AmmApi::BASE_SIZE as u32, abi_minor: 0, ..FakeTable::api() });

    // SAFETY: the table is leaked and holds a whole AmmApi
    let result = unsafe { ApiTable::new(api) };

    assert!(result.is_ok());
}

#[test]
fn a_client_refuses_another_major() {
    for major in [AMM_ABI_MAJOR - 1, AMM_ABI_MAJOR + 1] {
        let api = FakeTable::leak(AmmApi { abi_major: major, abi_minor: 0, ..FakeTable::api() });

        // SAFETY: the table is leaked, so it stays valid for the process
        let result = unsafe { ApiTable::new(api) };

        assert_eq!(result.err(), Some(LinkError::AbiMismatch { found: major }));
    }
}

// Where a 1.1, a 1.2 and a 1.4 manager's table ended. In 2.0 every slot is part of minor 0, so a table that ends
// at any of these is not a 2.x table.
const OLD_SIZES: [usize; 3] = [
    std::mem::offset_of!(AmmApi, log) + size_of::<usize>(),
    std::mem::offset_of!(AmmApi, texture_budget) + size_of::<usize>(),
    std::mem::offset_of!(AmmApi, register_menu_entry) + size_of::<usize>(),
];

#[test]
fn a_table_that_ends_before_the_last_function_of_2_0_is_abi_mismatch() {
    for size in OLD_SIZES.into_iter().chain([AmmApi::BASE_SIZE - size_of::<usize>()]) {
        let api = FakeTable::leak(AmmApi { size: size as u32, ..FakeTable::api() });

        // SAFETY: the table is leaked and holds a whole AmmApi, more than its declared size
        let result = unsafe { ApiTable::new(api) };

        assert_eq!(result.err(), Some(LinkError::AbiMismatch { found: AMM_ABI_MAJOR }), "size {size}");
    }
}
