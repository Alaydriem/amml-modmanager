use amm_abi::types::{AmmHandle, AmmStatus};
use amm_ffi::host::amm_host_take_fake_keys;
use amm_ffi::types::AmmHostFakeKey;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const KEY_W: u32 = 0x57;

fn send_key(handle: AmmHandle, vk: u32, down: u8) -> AmmStatus {
    // SAFETY: send_key takes plain values
    unsafe { TestHost::api().send_key.unwrap_or_else(|| panic!("send_key"))(handle, vk, down) }
}

fn input_mod(host: &TestHost, id: &str) -> TestClient {
    let descriptor = Descriptors::with_claims(id, &[("Input", "Shared")], "");
    TestClient::register(host, &descriptor, TestMesh::triangle())
}

#[test]
fn send_key_unknown_handle() {
    let _host = TestHost::new();
    let stale = AmmHandle { slot: 3, generation: 9 };

    assert_eq!(send_key(stale, KEY_W, 1), AmmStatus::UnknownHandle);
}

#[test]
fn send_key_down_2_is_invalid() {
    let host = TestHost::new();
    let client = input_mod(&host, "key-sender");

    assert_eq!(send_key(client.handle(), KEY_W, 2), AmmStatus::InvalidArgument);
    assert_eq!(send_key(client.handle(), KEY_W, 255), AmmStatus::InvalidArgument);
}

#[test]
fn watch_keys_null_with_count_is_null_pointer() {
    let host = TestHost::new();
    let client = input_mod(&host, "key-watcher");
    let watch = TestHost::api().watch_keys.unwrap_or_else(|| panic!("watch_keys"));

    // SAFETY: the null pointer is the case under test; no bytes are read
    let with_count = unsafe { watch(client.handle(), std::ptr::null(), 2) };
    // SAFETY: a null pointer with a count of 0 reads nothing
    let empty = unsafe { watch(client.handle(), std::ptr::null(), 0) };

    assert_eq!(with_count, AmmStatus::NullPointer);
    assert_eq!(empty, AmmStatus::Ok);
}

#[test]
fn take_fake_keys_drains_in_order() {
    let host = TestHost::new();
    let client = input_mod(&host, "key-presser");
    let mut out = [AmmHostFakeKey { vk: 0, down: 0 }; 8];
    // SAFETY: out holds 8 writable entries for the call
    let before = unsafe { amm_host_take_fake_keys(out.as_mut_ptr(), out.len()) };

    assert_eq!(send_key(client.handle(), KEY_W, 1), AmmStatus::Ok);
    assert_eq!(send_key(client.handle(), KEY_W, 0), AmmStatus::Ok);
    // SAFETY: out holds 8 writable entries for the call
    let taken = unsafe { amm_host_take_fake_keys(out.as_mut_ptr(), out.len()) };
    // SAFETY: out holds 8 writable entries for the call
    let again = unsafe { amm_host_take_fake_keys(out.as_mut_ptr(), out.len()) };

    assert_eq!(before, 0);
    assert_eq!(taken, 2);
    assert_eq!((out[0].vk, out[0].down), (KEY_W, 1));
    assert_eq!((out[1].vk, out[1].down), (KEY_W, 0));
    assert_eq!(again, 0);
}

#[test]
fn take_fake_keys_keeps_the_rest_for_the_next_call() {
    let host = TestHost::new();
    let client = input_mod(&host, "key-batcher");
    let mut out = [AmmHostFakeKey { vk: 0, down: 0 }; 1];
    assert_eq!(send_key(client.handle(), KEY_W, 1), AmmStatus::Ok);
    assert_eq!(send_key(client.handle(), KEY_W, 0), AmmStatus::Ok);

    // SAFETY: out holds 1 writable entry for the call
    let first = unsafe { amm_host_take_fake_keys(out.as_mut_ptr(), out.len()) };
    let first_key = (out[0].vk, out[0].down);
    // SAFETY: out holds 1 writable entry for the call
    let second = unsafe { amm_host_take_fake_keys(out.as_mut_ptr(), out.len()) };
    let second_key = (out[0].vk, out[0].down);
    // SAFETY: a null buffer with a capacity of 0 writes nothing
    let none = unsafe { amm_host_take_fake_keys(std::ptr::null_mut(), 0) };

    assert_eq!((first, first_key), (1, (KEY_W, 1)));
    assert_eq!((second, second_key), (1, (KEY_W, 0)));
    assert_eq!(none, 0);
}
