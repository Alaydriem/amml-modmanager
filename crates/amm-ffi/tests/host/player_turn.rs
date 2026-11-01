use amm_abi::types::AmmCapability;
use amm_ffi::host::amm_host_blocks_player_turn;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn look_descriptor(id: &str) -> String {
    let extra = "claim \"Input\" {
  mode = \"Shared\"
  reason = \"r\"
  block_player_turn = true
}
";
    Descriptors::with_claims(id, &[], extra)
}

#[test]
fn the_turn_is_blocked_only_while_a_flagged_input_holder_holds_input() {
    let host = TestHost::new();
    assert!(!amm_host_blocks_player_turn());
    let look = TestClient::register(&host, &look_descriptor("look"), TestMesh::triangle());

    assert!(!amm_host_blocks_player_turn());
    look.acquire(AmmCapability::Input);
    assert!(amm_host_blocks_player_turn());
    look.release_raw(AmmCapability::Input as u8);
    assert!(!amm_host_blocks_player_turn());
}

#[test]
fn the_turn_is_not_blocked_without_a_host() {
    assert!(!amm_host_blocks_player_turn());
}
