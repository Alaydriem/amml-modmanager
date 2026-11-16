use amm_abi::types::{AMM_MOVE_JUMP, AmmCapability, AmmStatus};
use amm_ffi::host::{amm_host_move_intent, amm_host_set_move_available};
use amm_ffi::types::AmmHostMove;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn input_holder(host: &TestHost, id: &str) -> TestClient {
    let descriptor = Descriptors::with_claims(id, &[("Input", "Shared")], "");
    let client = TestClient::register(host, &descriptor, TestMesh::triangle());
    client.acquire(AmmCapability::Input);
    client
}

fn send_move(client: &TestClient, forward: i32, left: i32, flags: u32) -> AmmStatus {
    // SAFETY: send_move takes plain values
    unsafe { TestHost::api().send_move.unwrap_or_else(|| panic!("send_move"))(client.handle(), forward, left, flags) }
}

// what the tick reads, with `player_ready` the game state it reports
fn tick_read(player_ready: bool) -> Option<AmmHostMove> {
    let mut out = AmmHostMove { forward_milli: 0, left_milli: 0, flags: 0 };
    // SAFETY: out is a writable local
    let held = unsafe { amm_host_move_intent(player_ready, &mut out) };
    held.then_some(out)
}

#[test]
fn the_tick_reads_the_held_intent_until_it_is_cleared() {
    let host = TestHost::new();
    let client = input_holder(&host, "mover");
    amm_host_set_move_available(true);
    assert_eq!(tick_read(true), None);

    assert_eq!(send_move(&client, 1000, -250, AMM_MOVE_JUMP), AmmStatus::Ok);
    let held = AmmHostMove { forward_milli: 1000, left_milli: -250, flags: AMM_MOVE_JUMP };
    assert_eq!(tick_read(true), Some(held));
    assert_eq!(tick_read(true), Some(held));

    assert_eq!(send_move(&client, 0, 0, 0), AmmStatus::Ok);
    assert_eq!(tick_read(true), None);
}

#[test]
fn a_tick_with_no_local_player_makes_send_move_not_ready() {
    let host = TestHost::new();
    let client = input_holder(&host, "mover-early");
    amm_host_set_move_available(true);
    tick_read(false);

    assert_eq!(send_move(&client, 1000, 0, 0), AmmStatus::NotReady);

    tick_read(true);
    assert_eq!(send_move(&client, 1000, 0, 0), AmmStatus::Ok);
}

#[test]
fn the_tick_writes_nothing_to_a_null_out() {
    let host = TestHost::new();
    let client = input_holder(&host, "mover-null");
    amm_host_set_move_available(true);
    tick_read(true);
    assert_eq!(send_move(&client, 1000, 0, 0), AmmStatus::Ok);

    // SAFETY: a null out is refused before any write
    assert!(!unsafe { amm_host_move_intent(true, std::ptr::null_mut()) });
}

#[test]
fn without_the_move_entries_send_move_is_unsupported() {
    let host = TestHost::new();
    let client = input_holder(&host, "mover-old-game");
    tick_read(true);

    assert_eq!(send_move(&client, 1000, 0, 0), AmmStatus::Unsupported);

    amm_host_set_move_available(true);
    assert_eq!(send_move(&client, 1000, 0, 0), AmmStatus::Ok);
}
