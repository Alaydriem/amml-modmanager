use amm_abi::types::{AMM_ABI_MAJOR, AmmCapability, AmmHandle, AmmStatus};

use amm_ffi::host::amm_host_game_point;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

struct PictureRig;

impl PictureRig {
    fn set_viewport(handle: AmmHandle, rect: Option<[f32; 4]>) -> AmmStatus {
        let call = TestHost::api().set_game_viewport.unwrap_or_else(|| panic!("set_game_viewport"));
        let pointer = rect.as_ref().map_or(std::ptr::null(), |rect| rect as *const [f32; 4]);
        // SAFETY: the rect lives for the call, or the pointer is null
        unsafe { call(handle, pointer) }
    }

    fn game_point(x: f32, y: f32) -> Option<(f32, f32)> {
        let (mut out_x, mut out_y) = (-1.0, -1.0);
        // SAFETY: both out pointers live for the call
        let mapped = unsafe { amm_host_game_point(x, y, TestHost::WIDTH, TestHost::HEIGHT, &mut out_x, &mut out_y) };
        mapped.then_some((out_x, out_y))
    }

    fn holder(host: &TestHost, id: &str) -> TestClient {
        let client = TestClient::register(host, &Descriptors::overlay(id), TestMesh::triangle());
        client.acquire(AmmCapability::Overlay);
        client.reply_for_layer(1, (true, false, 0, [0.0, 0.0, 1920.0, 1080.0]));
        host.frame(0);
        client
    }

    fn game_picture(handle: AmmHandle) -> (AmmStatus, u32, u32) {
        let call = TestHost::api().game_picture.unwrap_or_else(|| panic!("game_picture"));
        let (mut width, mut height) = (7, 7);
        // SAFETY: both out pointers live for the call
        let status = unsafe { call(handle, &mut width, &mut height) };
        (status, width, height)
    }
}

#[test]
fn the_overlay_holder_gets_the_back_buffer_size_of_the_last_present() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("picture-holder"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    host.frame(0);

    assert_eq!(PictureRig::game_picture(client.handle()), (AmmStatus::Ok, TestHost::WIDTH, TestHost::HEIGHT));
}

#[test]
fn a_mod_that_does_not_hold_overlay_is_not_permitted_and_nothing_is_written() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("picture-no-lease"), TestMesh::triangle());
    host.frame(0);

    assert_eq!(PictureRig::game_picture(client.handle()), (AmmStatus::NotPermitted, 7, 7));
}

#[test]
fn before_the_first_present_the_size_is_zero() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("picture-early"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);

    assert_eq!(PictureRig::game_picture(client.handle()), (AmmStatus::Ok, 0, 0));
}

#[test]
fn a_null_out_pointer_is_null_pointer() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("picture-null"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let call = TestHost::api().game_picture.unwrap_or_else(|| panic!("game_picture"));
    let mut height = 0;

    // SAFETY: a null pointer is refused before any write
    let status = unsafe { call(client.handle(), std::ptr::null_mut(), &mut height) };

    assert_eq!(status, AmmStatus::NullPointer);
}

#[test]
fn a_stale_handle_is_unknown() {
    let _host = TestHost::new();

    assert_eq!(PictureRig::game_picture(AmmHandle { slot: 9, generation: 99 }).0, AmmStatus::UnknownHandle);
}

#[test]
fn the_game_picture_slot_is_filled() {
    let table = amm_ffi::api::amm_api_for(AMM_ABI_MAJOR);
    // SAFETY: a non-null table is the manager's static table
    let api = unsafe { table.as_ref() }.unwrap_or_else(|| panic!("table"));

    assert!(api.game_picture.is_some());
}

#[test]
fn a_point_in_the_viewport_of_the_overlay_holder_maps_to_the_full_window() {
    let host = TestHost::new();
    let client = PictureRig::holder(&host, "viewport-map");

    assert_eq!(PictureRig::set_viewport(client.handle(), Some([480.0, 270.0, 1440.0, 810.0])), AmmStatus::Ok);

    assert_eq!(PictureRig::game_point(960.0, 540.0), Some((960.0, 540.0)));
    assert_eq!(PictureRig::game_point(480.0, 270.0), Some((0.0, 0.0)));
    assert_eq!(PictureRig::game_point(100.0, 100.0), None);
}

#[test]
fn a_cleared_viewport_maps_nothing() {
    let host = TestHost::new();
    let client = PictureRig::holder(&host, "viewport-clear");
    PictureRig::set_viewport(client.handle(), Some([480.0, 270.0, 1440.0, 810.0]));

    assert_eq!(PictureRig::set_viewport(client.handle(), None), AmmStatus::Ok);

    assert_eq!(PictureRig::game_point(960.0, 540.0), None);
}

#[test]
fn the_viewport_of_a_mod_without_overlay_maps_nothing() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("viewport-no-lease"), TestMesh::triangle());
    host.frame(0);

    assert_eq!(PictureRig::set_viewport(client.handle(), Some([480.0, 270.0, 1440.0, 810.0])), AmmStatus::Ok);

    assert_eq!(PictureRig::game_point(960.0, 540.0), None);
}

#[test]
fn an_empty_viewport_is_invalid() {
    let host = TestHost::new();
    let client = PictureRig::holder(&host, "viewport-empty");

    assert_eq!(PictureRig::set_viewport(client.handle(), Some([10.0, 10.0, 10.0, 40.0])), AmmStatus::InvalidArgument);
}

#[test]
fn a_layer_of_another_mod_over_the_viewport_keeps_that_point_from_the_game() {
    let host = TestHost::new();
    let holder = PictureRig::holder(&host, "viewport-under");
    PictureRig::set_viewport(holder.handle(), Some([480.0, 270.0, 1440.0, 810.0]));
    let cover = TestClient::register(&host, &Descriptors::overlay("viewport-cover"), TestMesh::triangle());
    cover.acquire(AmmCapability::Overlay);
    cover.reply_for_layer(1, (true, false, 0, [900.0, 500.0, 1000.0, 600.0]));
    host.frame(1);

    assert_eq!(PictureRig::game_point(950.0, 550.0), None);
    assert_eq!(PictureRig::game_point(960.0, 400.0), Some((960.0, 260.0)));
}
