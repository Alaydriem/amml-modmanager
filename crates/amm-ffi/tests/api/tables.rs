use std::ptr;

use amm_abi::types::{
    AmmAcquireOutcome, AmmAcquireResult, AmmCallbacks, AmmCapability, AmmCoreTable, AmmEvent, AmmEventKind,
    AmmKeybindsTable, AmmLayersTable, AmmLeasesTable, AmmSettingsTable, AmmStatus,
};
use amm_ffi::api::ManagerTables;
use amml_api::ApiError;
use amml_api::testing::FakeGame;

use crate::support::{Descriptors, ManagerProvider, RivalRegistry, TestClient, TestHost, TestMesh};

// The byte of the `Input` capability before 3.0, now unused
const OLD_INPUT: u8 = 1;

fn set_setting(client: &TestClient, path: &str, value: &str) -> AmmStatus {
    let set_setting = TestHost::settings_table().set_setting.unwrap_or_else(|| panic!("set_setting"));
    // SAFETY: both texts live for the call
    unsafe { set_setting(client.handle(), path.as_ptr(), path.len(), value.as_ptr(), value.len()) }
}

fn panel_open() -> u8 {
    // SAFETY: panel_open takes no arguments
    unsafe { TestHost::core_table().panel_open.unwrap_or_else(|| panic!("panel_open"))() }
}

fn poll_count(client: &TestClient) -> usize {
    let blank =
        AmmEvent { kind: AmmEventKind::Enabled, capability: 0, other_mod: 0, phase: 0, code: 0, x: 0.0, y: 0.0 };
    let mut buffer = [blank; 16];
    let poll_events = TestHost::core_table().poll_events.unwrap_or_else(|| panic!("poll_events"));
    // SAFETY: the buffer has room for its length in events
    unsafe { poll_events(client.handle(), buffer.as_mut_ptr(), buffer.len()) }
}

#[test]
fn the_five_tables_are_provided_under_their_names() {
    let fake = FakeGame::new();

    fake.enable::<ManagerProvider>().expect("enable provides the tables");

    let registry = fake.ctx().registry();
    assert!(registry.require::<AmmCoreTable>().is_some_and(|table| ptr::eq(table, ManagerTables::CORE)));
    assert!(registry.require::<AmmSettingsTable>().is_some_and(|table| ptr::eq(table, ManagerTables::SETTINGS)));
    assert!(registry.require::<AmmKeybindsTable>().is_some_and(|table| ptr::eq(table, ManagerTables::KEYBINDS)));
    assert!(registry.require::<AmmLayersTable>().is_some_and(|table| ptr::eq(table, ManagerTables::LAYERS)));
    assert!(registry.require::<AmmLeasesTable>().is_some_and(|table| ptr::eq(table, ManagerTables::LEASES)));
}

#[test]
fn a_second_provide_is_refused() {
    let fake = FakeGame::new();
    let rival = RivalRegistry::new(&fake);

    let first = ManagerTables::provide_all(&rival.ctx_as(1).registry());
    let second = ManagerTables::provide_all(&rival.ctx_as(2).registry());

    assert_eq!(first, Ok(()));
    assert_eq!(second, Err(ApiError::Refused));
    assert!(rival.ctx_as(3).registry().require::<AmmLeasesTable>().is_some());
}

#[test]
fn register_with_a_config_dir_outside_the_mods_root_is_invalid_argument() {
    let host = TestHost::new();
    let descriptor = Descriptors::overlay("outside");

    let outside = TestClient::register_handle(&descriptor, &host.outside_dir());
    let inside = TestClient::register_handle(&descriptor, &host.next_mod_dir());

    assert_eq!(outside, Err(AmmStatus::InvalidArgument));
    assert!(inside.is_ok());
}

#[test]
fn a_mods_root_itself_is_not_a_mod_config_dir() {
    let host = TestHost::new();

    let root = TestClient::register_handle(&Descriptors::overlay("root"), &host.mods_root());

    assert_eq!(root, Err(AmmStatus::InvalidArgument));
}

#[test]
fn a_call_after_disable_gives_disabled() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("late"), TestMesh::triangle());
    assert_eq!(client.acquire(AmmCapability::Overlay).outcome, AmmAcquireOutcome::Granted);
    host.with_runtime(|runtime| runtime.manager_mut().set_panel_open(true));
    assert_eq!(panel_open(), 1);
    assert!(TestClient::holder_id(AmmCapability::Overlay as u8, 64).0 > 0);

    host.disable_tables();

    assert_eq!(set_setting(&client, "a.b", "1"), AmmStatus::Disabled);
    let again = TestClient::register_handle(&Descriptors::overlay("again"), &host.next_mod_dir());
    assert_eq!(again, Err(AmmStatus::Disabled));
    let denied = client.acquire(AmmCapability::Overlay);
    assert_eq!(denied, AmmAcquireResult { outcome: AmmAcquireOutcome::Denied, holder: AmmAcquireResult::NO_HOLDER });
    assert_eq!(panel_open(), 0);
    assert_eq!(poll_count(&client), 0);
    assert_eq!(TestClient::holder_id(AmmCapability::Overlay as u8, 64).0, 0);
    let (_, callbacks) = TestClient::detached(TestMesh::triangle());
    let set_callbacks = TestHost::layers_table().set_callbacks.unwrap_or_else(|| panic!("set_callbacks"));
    // SAFETY: the callbacks live for the call
    assert_eq!(unsafe { set_callbacks(client.handle(), &callbacks) }, AmmStatus::Disabled);
    assert_eq!(TestClient::unregister_handle(client.handle()), AmmStatus::Disabled);
    host.frame(16);

    host.destroy();

    assert_eq!(set_setting(&client, "a.b", "1"), AmmStatus::Disabled);
    assert_eq!(panel_open(), 0);
}

#[test]
fn a_new_host_opens_the_tables_again() {
    let host = TestHost::new();
    host.disable_tables();
    host.destroy();
    drop(host);

    let host = TestHost::new();

    let client = TestClient::try_register(&host, &Descriptors::overlay("back"), TestMesh::triangle());
    assert!(client.is_ok());
}

#[test]
fn input_is_no_longer_a_manager_capability() {
    let host = TestHost::new();
    let descriptor = Descriptors::with_claims("driver", &[("Input", "Exclusive"), ("Overlay", "Shared")], "");
    let client = TestClient::register(&host, &descriptor, TestMesh::triangle());
    client.poll();

    let result = client.acquire_raw(OLD_INPUT);

    assert_eq!(result, AmmAcquireResult { outcome: AmmAcquireOutcome::Denied, holder: AmmAcquireResult::NO_HOLDER });
    assert_eq!(TestClient::holder_id(OLD_INPUT, 64).0, 0);
    assert!(client.poll().is_empty());
    assert_eq!(client.acquire(AmmCapability::Overlay).outcome, AmmAcquireOutcome::Granted);
}

#[test]
fn a_mod_draws_only_after_it_sets_callbacks() {
    let host = TestHost::new();
    let config_dir = host.next_mod_dir();
    let handle = TestClient::register_handle(&Descriptors::overlay("late-callbacks"), &config_dir).expect("registers");
    let acquire = TestHost::leases_table().acquire.unwrap_or_else(|| panic!("acquire"));
    // SAFETY: acquire takes plain values
    assert_eq!(unsafe { acquire(handle, AmmCapability::Overlay as u8) }.outcome, AmmAcquireOutcome::Granted);
    let (state, callbacks) = TestClient::detached(TestMesh::triangle());
    host.frame(0);
    assert_eq!(state.frames.load(std::sync::atomic::Ordering::SeqCst), 0);

    let set_callbacks = TestHost::layers_table().set_callbacks.unwrap_or_else(|| panic!("set_callbacks"));
    // SAFETY: the callbacks and their state live for the rest of the test
    assert_eq!(unsafe { set_callbacks(handle, &callbacks) }, AmmStatus::Ok);
    host.frame(16);

    assert_eq!(state.frames.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(TestClient::unregister_handle(handle), AmmStatus::Ok);
}

#[test]
fn set_callbacks_refuses_null_and_a_size_below_the_struct() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("sizes"), TestMesh::triangle());
    let set_callbacks = TestHost::layers_table().set_callbacks.unwrap_or_else(|| panic!("set_callbacks"));
    let (_, mut callbacks) = TestClient::detached(TestMesh::triangle());
    callbacks.size = size_of::<AmmCallbacks>() as u32 - 8;

    // SAFETY: a null pointer is allowed and refused; the callbacks live for the call
    let (null, small) =
        unsafe { (set_callbacks(client.handle(), ptr::null()), set_callbacks(client.handle(), &callbacks)) };

    assert_eq!((null, small), (AmmStatus::NullPointer, AmmStatus::InvalidArgument));
}
