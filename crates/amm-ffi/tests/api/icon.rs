use amm_abi::types::{AMM_ABI_MAJOR, AmmHandle, AmmStatus};
use amm_core::structs::registry::ModIndex;

use crate::support::{Descriptors, PngFixture, TestClient, TestHost, TestMesh};

struct IconRig;

impl IconRig {
    fn set_icon(handle: AmmHandle, bytes: &[u8]) -> AmmStatus {
        let call = TestHost::api().set_icon.unwrap_or_else(|| panic!("set_icon"));
        // SAFETY: the slice holds its length in readable bytes for the call
        unsafe { call(handle, bytes.as_ptr(), bytes.len()) }
    }

    fn stored(host: &TestHost, handle: AmmHandle) -> Option<(u16, u16)> {
        let mut size = None;
        host.with_runtime(|runtime| {
            size =
                runtime.manager().icon(ModIndex(handle.slot as u8)).map(|entry| (entry.icon.width, entry.icon.height));
        });
        size
    }

    fn client(host: &TestHost, id: &str) -> TestClient {
        TestClient::register(host, &Descriptors::overlay(id), TestMesh::triangle())
    }
}

#[test]
fn valid_icon_is_ok_and_stored() {
    let host = TestHost::new();
    let client = IconRig::client(&host, "icon-valid");

    assert_eq!(IconRig::set_icon(client.handle(), &PngFixture::rgba(8, 4)), AmmStatus::Ok);

    assert_eq!(IconRig::stored(&host, client.handle()), Some((8, 4)));
}

#[test]
fn zero_length_clears() {
    let host = TestHost::new();
    let client = IconRig::client(&host, "icon-clear");
    assert_eq!(IconRig::set_icon(client.handle(), &PngFixture::rgba(4, 4)), AmmStatus::Ok);

    let call = TestHost::api().set_icon.unwrap_or_else(|| panic!("set_icon"));
    // SAFETY: a zero length reads no bytes, so a null pointer is allowed
    let status = unsafe { call(client.handle(), std::ptr::null(), 0) };

    assert_eq!(status, AmmStatus::Ok);
    assert_eq!(IconRig::stored(&host, client.handle()), None);
}

#[test]
fn null_with_length_is_null_pointer() {
    let host = TestHost::new();
    let client = IconRig::client(&host, "icon-null");

    let call = TestHost::api().set_icon.unwrap_or_else(|| panic!("set_icon"));
    // SAFETY: a null pointer is refused before any read
    let status = unsafe { call(client.handle(), std::ptr::null(), 16) };

    assert_eq!(status, AmmStatus::NullPointer);
}

#[test]
fn bad_png_is_invalid_argument_and_keeps_the_old_icon() {
    let host = TestHost::new();
    let client = IconRig::client(&host, "icon-keep");
    assert_eq!(IconRig::set_icon(client.handle(), &PngFixture::rgba(6, 6)), AmmStatus::Ok);

    assert_eq!(IconRig::set_icon(client.handle(), b"not a png at all"), AmmStatus::InvalidArgument);

    assert_eq!(IconRig::stored(&host, client.handle()), Some((6, 6)));
}

#[test]
fn unknown_handle_with_good_png_is_unknown_handle() {
    let host = TestHost::new();
    let _client = IconRig::client(&host, "icon-unknown");
    let stale = AmmHandle { slot: 30, generation: 7 };

    assert_eq!(IconRig::set_icon(stale, &PngFixture::rgba(4, 4)), AmmStatus::UnknownHandle);
}

#[test]
fn unknown_handle_with_bad_png_is_invalid_argument() {
    let host = TestHost::new();
    let _client = IconRig::client(&host, "icon-unknown-bad");
    let stale = AmmHandle { slot: 30, generation: 7 };

    assert_eq!(IconRig::set_icon(stale, b"not a png at all"), AmmStatus::InvalidArgument);
}

#[test]
fn the_icon_slot_is_filled() {
    let table = amm_ffi::api::amm_api_for(AMM_ABI_MAJOR);
    // SAFETY: a non-null table is the manager's static table
    let api = unsafe { table.as_ref() }.unwrap_or_else(|| panic!("table"));

    assert!(api.set_icon.is_some());
}
