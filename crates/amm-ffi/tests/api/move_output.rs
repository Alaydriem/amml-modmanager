use amm_abi::types::{AmmCapability, AmmHandle, AmmStatus};
use amm_ffi::host::{amm_host_move_intent, amm_host_set_move_available};
use amm_ffi::types::AmmHostMove;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn send_move(handle: AmmHandle, forward: i32, left: i32, flags: u32) -> AmmStatus {
    // SAFETY: send_move takes plain values
    unsafe { TestHost::api().send_move.unwrap_or_else(|| panic!("send_move"))(handle, forward, left, flags) }
}

#[test]
fn send_move_unknown_handle() {
    let _host = TestHost::new();
    let stale = AmmHandle { slot: 3, generation: 9 };

    assert_eq!(send_move(stale, 1000, 0, 0), AmmStatus::UnknownHandle);
}

#[test]
fn send_move_bad_axis_or_flag_is_invalid() {
    let host = TestHost::new();
    let descriptor = Descriptors::with_claims("mover-bad", &[("Input", "Shared")], "");
    let client = TestClient::register(&host, &descriptor, TestMesh::triangle());
    client.acquire(AmmCapability::Input);

    assert_eq!(send_move(client.handle(), 1001, 0, 0), AmmStatus::InvalidArgument);
    assert_eq!(send_move(client.handle(), 0, -1001, 0), AmmStatus::InvalidArgument);
    assert_eq!(send_move(client.handle(), 0, 0, 8), AmmStatus::InvalidArgument);
}

#[test]
fn send_move_needs_the_input_lease() {
    let host = TestHost::new();
    let descriptor = Descriptors::with_claims("mover-unleased", &[("Input", "Shared")], "");
    let client = TestClient::register(&host, &descriptor, TestMesh::triangle());
    amm_host_set_move_available(true);
    let mut out = AmmHostMove { forward_milli: 0, left_milli: 0, flags: 0 };
    // SAFETY: out is a writable local
    unsafe { amm_host_move_intent(true, &mut out) };

    assert_eq!(send_move(client.handle(), 0, 0, 0), AmmStatus::Ok);
    assert_eq!(send_move(client.handle(), 1000, 0, 0), AmmStatus::NotPermitted);
    client.acquire(AmmCapability::Input);
    assert_eq!(send_move(client.handle(), 1000, 0, 0), AmmStatus::Ok);
}
