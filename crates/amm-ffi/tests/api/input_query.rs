use amm_abi::types::AmmHandle;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn panel_open(handle: AmmHandle) -> u8 {
    // SAFETY: panel_open takes plain values
    unsafe { TestHost::api().panel_open.unwrap_or_else(|| panic!("panel_open"))(handle) }
}

#[test]
fn panel_open_follows_the_panel() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("panel-reader"), TestMesh::triangle());
    let closed = panel_open(client.handle());
    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(true));
    let open = panel_open(client.handle());
    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(false));

    assert_eq!((closed, open, panel_open(client.handle())), (0, 1, 0));
}

#[test]
fn panel_open_is_zero_for_a_stale_handle() {
    let host = TestHost::new();
    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(true));
    let stale = AmmHandle { slot: 3, generation: 9 };

    assert_eq!(panel_open(stale), 0);
}
