use std::path::{Path, PathBuf};

use amm_abi::types::{AmmCapability, AmmHandle, AmmStatus};

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

fn request(handle: AmmHandle, path: &str) -> (AmmStatus, u32) {
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    // SAFETY: the path and the out sequence live for the call
    let status = unsafe { call(handle, path.as_ptr(), path.len(), &mut sequence) };
    (status, sequence)
}

fn shot(host: &TestHost, name: &str) -> String {
    let dir = host.game_dir().join("shots");
    std::fs::create_dir_all(&dir).unwrap_or_else(|error| panic!("shots dir: {error}"));
    dir.join(name).display().to_string()
}

fn overlay_client(host: &TestHost, id: &str) -> TestClient {
    let client = TestClient::register(host, &Descriptors::overlay(id), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    client
}

fn held_by_other(handle: AmmHandle, capability: u8) -> u8 {
    // SAFETY: held_by_other takes plain values
    unsafe { TestHost::api().held_by_other.unwrap_or_else(|| panic!("held_by_other"))(handle, capability) }
}

#[test]
fn a_mod_with_an_overlay_lease_gets_a_sequence_for_a_good_path() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");

    let (status, sequence) = request(client.handle(), &shot(&host, "a.png"));

    assert_eq!(status, AmmStatus::Ok);
    assert_ne!(sequence, 0);
}

#[test]
fn a_null_path_or_a_null_out_sequence_is_null_pointer() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    let path = shot(&host, "a.png");
    let mut sequence = 0;

    // SAFETY: the path pointer is null on purpose; the out sequence lives for the call
    let null_path = unsafe { call(client.handle(), std::ptr::null(), 8, &mut sequence) };
    // SAFETY: the path pointer is null on purpose, with a length past the limit; the out sequence lives for the call
    let null_long_path = unsafe { call(client.handle(), std::ptr::null(), 5000, &mut sequence) };
    // SAFETY: the path lives for the call; the out sequence is null on purpose
    let null_out = unsafe { call(client.handle(), path.as_ptr(), path.len(), std::ptr::null_mut()) };

    assert_eq!(null_path, AmmStatus::NullPointer);
    assert_eq!(null_long_path, AmmStatus::NullPointer);
    assert_eq!(null_out, AmmStatus::NullPointer);
    assert_eq!(sequence, 0);
}

#[test]
fn an_oversized_relative_non_png_or_non_utf8_path_is_invalid_argument() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let long = format!("{}{}", shot(&host, ""), "a".repeat(5000) + ".png");
    let call = TestHost::api().request_screenshot.unwrap_or_else(|| panic!("request_screenshot"));
    let bad_utf8 = [0xC3u8, 0x28, b'.', b'p', b'n', b'g'];
    let mut sequence = 0;

    let oversized = request(client.handle(), &long).0;
    let relative = request(client.handle(), "shot.png").0;
    let not_png = request(client.handle(), &shot(&host, "a.jpg")).0;
    let dot_dot = request(client.handle(), &format!("{}{}", shot(&host, ""), "..\\a.png")).0;
    // SAFETY: the bytes and the out sequence live for the call
    let not_utf8 = unsafe { call(client.handle(), bad_utf8.as_ptr(), bad_utf8.len(), &mut sequence) };

    for status in [oversized, relative, not_png, dot_dot, not_utf8] {
        assert_eq!(status, AmmStatus::InvalidArgument);
    }
}

#[test]
fn a_path_whose_parent_folder_is_missing_is_invalid_argument() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let missing: PathBuf = host.game_dir().join("no-such-folder").join("a.png");

    let (status, _) = request(client.handle(), &missing.display().to_string());

    assert_eq!(status, AmmStatus::InvalidArgument);
    assert!(!missing.parent().is_some_and(Path::exists));
}

#[test]
fn a_mod_without_a_hud_or_overlay_lease_is_not_permitted() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("shooter"), TestMesh::triangle());

    let missing = host.game_dir().join("no-such-folder").join("a.png").display().to_string();

    let (status, _) = request(client.handle(), &shot(&host, "a.png"));
    let missing_folder = request(client.handle(), &missing).0;
    let relative = request(client.handle(), "shot.png").0;

    assert_eq!(status, AmmStatus::NotPermitted);
    assert_eq!(missing_folder, AmmStatus::NotPermitted);
    assert_eq!(relative, AmmStatus::NotPermitted);
}

#[test]
fn a_second_request_while_one_is_in_flight_is_busy() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");

    let first = request(client.handle(), &shot(&host, "a.png"));
    let second = request(client.handle(), &shot(&host, "b.png"));

    assert_eq!(first.0, AmmStatus::Ok);
    assert_eq!(second.0, AmmStatus::Busy);
}

#[test]
fn a_stale_handle_is_unknown_handle() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "shooter");
    let stale = client.handle();
    client.unregister();

    let (status, _) = request(stale, &shot(&host, "a.png"));

    assert_eq!(status, AmmStatus::UnknownHandle);
}

#[test]
fn held_by_other_names_a_lease_of_another_mod_and_not_the_callers_own() {
    let host = TestHost::new();
    let holder = overlay_client(&host, "holder");
    let other = TestClient::register(&host, &Descriptors::overlay("other"), TestMesh::triangle());

    let for_other = held_by_other(other.handle(), AmmCapability::Overlay as u8);
    let for_holder = held_by_other(holder.handle(), AmmCapability::Overlay as u8);
    let free_capability = held_by_other(other.handle(), AmmCapability::Camera as u8);

    assert_eq!((for_other, for_holder, free_capability), (1, 0, 0));
}

#[test]
fn held_by_other_is_zero_for_a_bad_capability_or_a_stale_handle() {
    let host = TestHost::new();
    let _holder = overlay_client(&host, "holder");
    let mut other = TestClient::register(&host, &Descriptors::overlay("other"), TestMesh::triangle());
    let bad_capability = held_by_other(other.handle(), 42);
    let stale = other.handle();
    other.unregister();

    let stale_answer = held_by_other(stale, AmmCapability::Overlay as u8);

    assert_eq!((bad_capability, stale_answer), (0, 0));
}

#[test]
fn texture_budget_gives_the_grant_the_descriptor_asked_for() {
    let host = TestHost::new();
    let descriptor = Descriptors::with_claims(
        "budget",
        &[("Overlay", "Shared")],
        "texture_budget_mb = 64\ntexture_budget_count = 10\n",
    );
    let client = TestClient::register(&host, &descriptor, TestMesh::triangle());
    let call = TestHost::api().texture_budget.unwrap_or_else(|| panic!("texture_budget"));
    let (mut bytes, mut count) = (0u64, 0u32);

    // SAFETY: both out pointers live for the call
    let status = unsafe { call(client.handle(), &mut bytes, &mut count) };

    assert_eq!(status, AmmStatus::Ok);
    assert_eq!((bytes, count), (64 * 1024 * 1024, 10));
}

#[test]
fn texture_budget_refuses_null_out_pointers_and_a_stale_handle() {
    let host = TestHost::new();
    let mut client = overlay_client(&host, "budget");
    let call = TestHost::api().texture_budget.unwrap_or_else(|| panic!("texture_budget"));
    let (mut bytes, mut count) = (0u64, 0u32);

    // SAFETY: the bytes pointer is null on purpose; count lives for the call
    let null_bytes = unsafe { call(client.handle(), std::ptr::null_mut(), &mut count) };
    // SAFETY: bytes lives for the call; the count pointer is null on purpose
    let null_count = unsafe { call(client.handle(), &mut bytes, std::ptr::null_mut()) };
    let stale = client.handle();
    client.unregister();
    // SAFETY: both out pointers live for the call
    let stale_status = unsafe { call(stale, &mut bytes, &mut count) };

    assert_eq!(null_bytes, AmmStatus::NullPointer);
    assert_eq!(null_count, AmmStatus::NullPointer);
    assert_eq!(stale_status, AmmStatus::UnknownHandle);
    assert_eq!((bytes, count), (0, 0));
}

fn request_layers(handle: AmmHandle, path: &[u8], layers: u8) -> (AmmStatus, u32) {
    let mut sequence = 0;
    let call = TestHost::api().request_screenshot_layers.unwrap_or_else(|| panic!("request_screenshot_layers"));
    // SAFETY: the path and the out sequence live for the call
    let status = unsafe { call(handle, path.as_ptr(), path.len(), layers, &mut sequence) };
    (status, sequence)
}

#[test]
fn a_layers_value_other_than_0_or_1_is_invalid_argument() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let path = shot(&host, "a.png");

    let refused: Vec<AmmStatus> =
        [2, 3, 0x80, 0xFF].map(|layers| request_layers(client.handle(), path.as_bytes(), layers).0).to_vec();
    let with_layers = request_layers(client.handle(), path.as_bytes(), 1);

    assert!(refused.iter().all(|status| *status == AmmStatus::InvalidArgument), "{refused:?}");
    assert_eq!(with_layers.0, AmmStatus::Ok);
    assert_ne!(with_layers.1, 0);
}

#[test]
fn request_screenshot_layers_with_a_null_path_or_out_sequence_is_null_pointer() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let call = TestHost::api().request_screenshot_layers.unwrap_or_else(|| panic!("request_screenshot_layers"));
    let path = shot(&host, "a.png");
    let mut sequence = 0;

    // SAFETY: the path pointer is null on purpose; the out sequence lives for the call
    let null_path = unsafe { call(client.handle(), std::ptr::null(), 8, 1, &mut sequence) };
    // SAFETY: the path pointer is null on purpose, with a bad layers value; the out sequence lives for the call
    let null_path_bad_layers = unsafe { call(client.handle(), std::ptr::null(), 8, 9, &mut sequence) };
    // SAFETY: the path lives for the call; the out sequence is null on purpose
    let null_out = unsafe { call(client.handle(), path.as_ptr(), path.len(), 1, std::ptr::null_mut()) };

    assert_eq!(null_path, AmmStatus::NullPointer);
    assert_eq!(null_path_bad_layers, AmmStatus::NullPointer);
    assert_eq!(null_out, AmmStatus::NullPointer);
    assert_eq!(sequence, 0);
}

#[test]
fn request_screenshot_layers_follows_the_path_lease_and_busy_rules_of_request_screenshot() {
    let host = TestHost::new();
    let client = overlay_client(&host, "shooter");
    let unleased = TestClient::register(&host, &Descriptors::overlay("unleased"), TestMesh::triangle());
    let long = format!("{}{}", shot(&host, ""), "a".repeat(5000) + ".png");
    let missing = host.game_dir().join("no-such-folder").join("a.png").display().to_string();
    let bad_utf8 = [0xC3u8, 0x28, b'.', b'p', b'n', b'g'];

    let invalid = [
        request_layers(client.handle(), long.as_bytes(), 1).0,
        request_layers(client.handle(), b"shot.png", 1).0,
        request_layers(client.handle(), shot(&host, "a.jpg").as_bytes(), 1).0,
        request_layers(client.handle(), format!("{}{}", shot(&host, ""), "..\\a.png").as_bytes(), 1).0,
        request_layers(client.handle(), missing.as_bytes(), 1).0,
        request_layers(client.handle(), &bad_utf8, 1).0,
    ];
    let not_permitted = request_layers(unleased.handle(), shot(&host, "a.png").as_bytes(), 1).0;
    let first = request_layers(client.handle(), shot(&host, "a.png").as_bytes(), 1).0;
    let second_layers = request_layers(client.handle(), shot(&host, "b.png").as_bytes(), 1).0;
    let second_game = request(client.handle(), &shot(&host, "b.png")).0;

    assert!(invalid.iter().all(|status| *status == AmmStatus::InvalidArgument), "{invalid:?}");
    assert_eq!(not_permitted, AmmStatus::NotPermitted);
    assert_eq!(first, AmmStatus::Ok);
    assert_eq!(second_layers, AmmStatus::Busy);
    assert_eq!(second_game, AmmStatus::Busy);
}
