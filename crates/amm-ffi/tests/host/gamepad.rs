use amm_abi::types::AmmCapability;
use amm_ffi::host::amm_host_blocks_gamepad;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn input_descriptor(id: &str, full_block: bool) -> String {
    let flag = if full_block { "  full_block = true\n" } else { "" };
    let extra = format!("claim \"Input\" {{\n  mode = \"Exclusive\"\n  reason = \"r\"\n{flag}}}\n");
    Descriptors::with_claims(id, &[], &extra)
}

#[test]
fn the_gamepad_is_blocked_only_while_a_full_block_holder_holds_input() {
    let host = TestHost::new();
    let plain = TestClient::register(&host, &input_descriptor("plain", false), TestMesh::triangle());
    let headless = TestClient::register(&host, &input_descriptor("headless", true), TestMesh::triangle());

    plain.acquire(AmmCapability::Input);
    assert!(!amm_host_blocks_gamepad());
    plain.release_raw(AmmCapability::Input as u8);

    headless.acquire(AmmCapability::Input);
    assert!(amm_host_blocks_gamepad());
    headless.release_raw(AmmCapability::Input as u8);
    assert!(!amm_host_blocks_gamepad());
}

#[test]
fn the_gamepad_is_not_blocked_without_a_host() {
    assert!(!amm_host_blocks_gamepad());
}
