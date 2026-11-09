use amm_abi::types::{AmmHandle, AmmStatus};
use amm_ffi::host::amm_host_take_fake_pointer;
use amm_ffi::types::AmmHostFakePointer;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const LEFT: u32 = 1;
const WHEEL: u32 = 4;
const TURN_YAW: u32 = 5;
const TURN_PITCH: u32 = 6;

fn send_pointer(handle: AmmHandle, op: u32, value: i32) -> AmmStatus {
    // SAFETY: send_pointer takes plain values
    unsafe { TestHost::api().send_pointer.unwrap_or_else(|| panic!("send_pointer"))(handle, op, value) }
}

fn input_mod(host: &TestHost, id: &str) -> TestClient {
    let descriptor = Descriptors::with_claims(id, &[("Input", "Shared")], "");
    TestClient::register(host, &descriptor, TestMesh::triangle())
}

fn take(out: &mut [AmmHostFakePointer]) -> usize {
    // SAFETY: out is a writable slice with room for out.len() entries
    unsafe { amm_host_take_fake_pointer(out.as_mut_ptr(), out.len()) }
}

#[test]
fn send_pointer_unknown_handle() {
    let _host = TestHost::new();
    let stale = AmmHandle { slot: 3, generation: 9 };

    assert_eq!(send_pointer(stale, LEFT, 1), AmmStatus::UnknownHandle);
}

#[test]
fn send_pointer_unknown_op_or_bad_button_value_is_invalid() {
    let host = TestHost::new();
    let client = input_mod(&host, "pointer-sender");

    assert_eq!(send_pointer(client.handle(), 0, 1), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), 7, 1), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), LEFT, 2), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), LEFT, -1), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), WHEEL, 1000), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), TURN_YAW, 360_001), AmmStatus::InvalidArgument);
    assert_eq!(send_pointer(client.handle(), TURN_PITCH, -360_001), AmmStatus::InvalidArgument);
}

#[test]
fn take_fake_pointer_drains_in_order_and_keeps_the_rest() {
    let host = TestHost::new();
    let client = input_mod(&host, "pointer-taker");
    let mut drain = [AmmHostFakePointer { op: 0, value: 0 }; 8];
    take(&mut drain);
    assert_eq!(send_pointer(client.handle(), LEFT, 1), AmmStatus::Ok);
    assert_eq!(send_pointer(client.handle(), WHEEL, -2), AmmStatus::Ok);

    let mut one = [AmmHostFakePointer { op: 0, value: 0 }; 1];
    assert_eq!(take(&mut one), 1);
    assert_eq!(one[0], AmmHostFakePointer { op: LEFT, value: 1 });
    assert_eq!(take(&mut one), 1);
    assert_eq!(one[0], AmmHostFakePointer { op: WHEEL, value: -2 });
    // SAFETY: a null pointer with a capacity of 0 writes nothing
    assert_eq!(unsafe { amm_host_take_fake_pointer(std::ptr::null_mut(), 0) }, 0);
    assert_eq!(take(&mut one), 0);
}

#[test]
fn turns_reach_the_host_as_one_pending_value_per_axis() {
    let host = TestHost::new();
    let client = input_mod(&host, "pointer-turner");
    let mut drain = [AmmHostFakePointer { op: 0, value: 0 }; 8];
    take(&mut drain);
    assert_eq!(send_pointer(client.handle(), TURN_YAW, 1500), AmmStatus::Ok);
    assert_eq!(send_pointer(client.handle(), TURN_YAW, 500), AmmStatus::Ok);
    assert_eq!(send_pointer(client.handle(), TURN_PITCH, -250), AmmStatus::Ok);

    assert_eq!(take(&mut drain), 2);

    assert_eq!(drain[0], AmmHostFakePointer { op: TURN_PITCH, value: -250 });
    assert_eq!(drain[1], AmmHostFakePointer { op: TURN_YAW, value: 2000 });
    assert_eq!(take(&mut drain), 0);
}
