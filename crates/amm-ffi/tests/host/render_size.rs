use amm_abi::types::{AMM_ABI_MAJOR, AmmCapability, AmmHandle, AmmRenderSize, AmmRenderSizeState, AmmStatus};
use amm_core::services::render::RenderSizeBook;
use amm_core::structs::registry::ModIndex;
use amm_ffi::host::{
    amm_host_create, amm_host_game_point, amm_host_render_size, amm_host_render_size_shutdown, amm_host_window_changed,
    amm_host_window_size,
};
use amm_ffi::types::AmmHostFrame;

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const UHD: AmmRenderSize = AmmRenderSize { width: 3840, height: 2160 };
const WINDOW: (u32, u32) = (1600, 900);
const SMALL: (u32, u32) = (1280, 720);
const R8G8B8A8_UNORM: u32 = 28;

struct SizeRig;

impl SizeRig {
    fn holder(host: &TestHost, id: &str) -> TestClient {
        let descriptor = Descriptors::with_claims(id, &[("RenderSize", "Exclusive")], "");
        let client = TestClient::register(host, &descriptor, TestMesh::triangle());
        client.acquire(AmmCapability::RenderSize);
        host.frame_sized(0, WINDOW, WINDOW);
        client
    }

    fn set(handle: AmmHandle, size: Option<AmmRenderSize>) -> AmmStatus {
        let call = TestHost::api().set_render_size.unwrap_or_else(|| panic!("set_render_size"));
        let pointer = size.as_ref().map_or(std::ptr::null(), |size| size as *const AmmRenderSize);
        // SAFETY: the size lives for the call, or the pointer is null
        unsafe { call(handle, pointer) }
    }

    fn state(handle: AmmHandle) -> (AmmStatus, AmmRenderSizeState) {
        let call = TestHost::api().render_size.unwrap_or_else(|| panic!("render_size"));
        let none = AmmRenderSize { width: 7, height: 7 };
        let mut state =
            AmmRenderSizeState { size: size_of::<AmmRenderSizeState>() as u32, wanted: none, actual: none, format: 7 };
        // SAFETY: the out state lives for the call
        let status = unsafe { call(handle, &mut state) };
        (status, state)
    }

    // the size a game ResizeBuffers for `asked` gets, and whether a size is forced
    fn resize(asked: (u32, u32)) -> (bool, (u32, u32)) {
        let (mut width, mut height) = (0, 0);
        // SAFETY: both out pointers live for the call
        let forced = unsafe { amm_host_render_size(asked.0, asked.1, &mut width, &mut height) };
        (forced, (width, height))
    }

    // a holder of RenderSize and Overlay, its Overlay layer over the whole 3840x2160 back buffer
    fn picture_holder(host: &TestHost, id: &str) -> TestClient {
        let claims = [("Overlay", "Shared"), ("RenderSize", "Exclusive")];
        let client = TestClient::register(host, &Descriptors::with_claims(id, &claims, ""), TestMesh::triangle());
        client.acquire(AmmCapability::Overlay);
        client.acquire(AmmCapability::RenderSize);
        client.reply_for_layer(1, (true, false, 0, [0.0, 0.0, 3840.0, 2160.0]));
        host.frame_sized(0, (3840, 2160), WINDOW);
        client
    }

    fn set_viewport(handle: AmmHandle, rect: [f32; 4]) {
        let call = TestHost::api().set_game_viewport.unwrap_or_else(|| panic!("set_game_viewport"));
        // SAFETY: the rect lives for the call
        assert_eq!(unsafe { call(handle, &rect) }, AmmStatus::Ok);
    }

    // the point the game's mouse feed gets for the real cursor at back-buffer `point`, with a `client` window
    fn game_point(point: (f32, f32), client: (u32, u32)) -> Option<(f32, f32)> {
        let (mut x, mut y) = (-1.0, -1.0);
        // SAFETY: both out pointers live for the call
        let moved = unsafe { amm_host_game_point(point.0, point.1, client.0, client.1, &mut x, &mut y) };
        moved.then_some((x, y))
    }

    fn shutdown(client: (u32, u32)) -> Option<(u32, u32)> {
        let (mut width, mut height) = (0, 0);
        // SAFETY: both out pointers live for the call
        let due = unsafe { amm_host_render_size_shutdown(client.0, client.1, &mut width, &mut height) };
        due.then_some((width, height))
    }

    fn window(minimised: bool, asked: (u32, u32)) -> (bool, (u32, u32)) {
        let (mut width, mut height) = (0, 0);
        // SAFETY: both out pointers live for the call
        let forced = unsafe { amm_host_window_size(minimised, asked.0, asked.1, &mut width, &mut height) };
        (forced, (width, height))
    }

    fn message(frame: &AmmHostFrame) -> Option<(u32, u32)> {
        let size = (frame.size_message_width, frame.size_message_height);
        (size != (0, 0)).then_some(size)
    }

    fn size(size: AmmRenderSize) -> (u32, u32) {
        (size.width, size.height)
    }
}

#[test]
fn setting_a_size_makes_the_next_host_frame_report_it_as_wanted() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-set");

    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::Ok);
    let first = host.frame_sized(1, WINDOW, WINDOW);
    let (status, before) = SizeRig::state(client.handle());

    assert_eq!(SizeRig::message(&first), Some((3840, 2160)));
    assert_eq!(status, AmmStatus::Ok);
    assert_eq!(SizeRig::size(before.wanted), (3840, 2160));
    assert_eq!(SizeRig::size(before.actual), WINDOW);

    let resized = host.frame_sized(2, (3840, 2160), WINDOW);
    let (_, after) = SizeRig::state(client.handle());

    assert_eq!(SizeRig::message(&resized), None);
    assert_eq!(SizeRig::size(after.actual), (3840, 2160));
    assert_eq!(after.format, R8G8B8A8_UNORM);
}

#[test]
fn a_game_resize_to_1280x720_while_set_is_rewritten_to_the_forced_size() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-rewrite");
    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));

    SizeRig::set(client.handle(), Some(UHD));

    assert_eq!(SizeRig::resize(SMALL), (true, (3840, 2160)));
    assert_eq!(SizeRig::resize((0, 0)), (true, (3840, 2160)));
}

#[test]
fn a_window_resize_while_set_sends_the_forced_size_again() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-reapply");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    let resized = host.frame_sized(2, (3840, 2160), SMALL);

    assert_eq!(SizeRig::message(&resized), Some((3840, 2160)));
    assert_eq!(SizeRig::message(&host.frame_sized(3, (3840, 2160), SMALL)), None);
}

#[test]
fn none_restores_the_last_size_the_game_asked_for() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-none");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);
    host.frame_sized(2, (3840, 2160), SMALL);
    assert_eq!(SizeRig::resize(SMALL), (true, (3840, 2160)));

    assert_eq!(SizeRig::set(client.handle(), None), AmmStatus::Ok);
    let restored = host.frame_sized(3, (3840, 2160), SMALL);
    let (_, state) = SizeRig::state(client.handle());

    assert_eq!(SizeRig::message(&restored), Some(SMALL));
    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
    assert_eq!(SizeRig::size(state.wanted), (0, 0));
    assert_eq!(SizeRig::message(&host.frame_sized(4, SMALL, SMALL)), None);
}

#[test]
fn dropping_the_lease_restores_it_too() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-release");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    client.release_raw(AmmCapability::RenderSize as u8);
    let restored = host.frame_sized(2, (3840, 2160), WINDOW);

    assert_eq!(SizeRig::message(&restored), Some(WINDOW));
    assert_eq!(SizeRig::resize(WINDOW), (false, WINDOW));
    assert_eq!(SizeRig::state(client.handle()).0, AmmStatus::NotPermitted);
}

#[test]
fn unregistering_the_holder_restores_the_size() {
    let host = TestHost::new();
    let mut client = SizeRig::holder(&host, "size-unregister");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    client.unregister();
    let restored = host.frame_sized(2, (3840, 2160), WINDOW);

    assert_eq!(SizeRig::message(&restored), Some(WINDOW));
    assert_eq!(SizeRig::resize(WINDOW), (false, WINDOW));
}

#[test]
fn a_restore_waits_while_the_window_is_minimised() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-minimised");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    SizeRig::set(client.handle(), None);

    assert_eq!(SizeRig::message(&host.frame_sized(2, (3840, 2160), (0, 0))), None);
    assert_eq!(SizeRig::message(&host.frame_sized(3, (3840, 2160), (0, 0))), None);
    assert_eq!(SizeRig::message(&host.frame_sized(4, (3840, 2160), WINDOW)), Some(WINDOW));
}

#[test]
fn a_mod_without_the_lease_is_not_permitted_and_forces_nothing() {
    let host = TestHost::new();
    let descriptor = Descriptors::with_claims("size-no-lease", &[("RenderSize", "Exclusive")], "");
    let client = TestClient::register(&host, &descriptor, TestMesh::triangle());

    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::NotPermitted);
    assert_eq!(SizeRig::state(client.handle()).0, AmmStatus::NotPermitted);
    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
}

#[test]
fn a_size_out_of_range_is_invalid_and_a_stale_handle_is_unknown() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-range");

    assert_eq!(
        SizeRig::set(client.handle(), Some(AmmRenderSize { width: 0, height: 2160 })),
        AmmStatus::InvalidArgument
    );
    assert_eq!(
        SizeRig::set(client.handle(), Some(AmmRenderSize { width: 3840, height: 16385 })),
        AmmStatus::InvalidArgument
    );
    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
    assert_eq!(SizeRig::set(AmmHandle { slot: 9, generation: 99 }, Some(UHD)), AmmStatus::UnknownHandle);
}

#[test]
fn a_null_state_pointer_is_null_pointer() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-null");
    let call = TestHost::api().render_size.unwrap_or_else(|| panic!("render_size"));

    // SAFETY: a null pointer is refused before any write
    let status = unsafe { call(client.handle(), std::ptr::null_mut()) };

    assert_eq!(status, AmmStatus::NullPointer);
}

#[test]
fn the_render_size_slots_are_filled() {
    let table = amm_ffi::api::amm_api_for(AMM_ABI_MAJOR);
    // SAFETY: a non-null table is the manager's static table
    let api = unsafe { table.as_ref() }.unwrap_or_else(|| panic!("table"));

    assert!(api.set_render_size.is_some());
    assert!(api.render_size.is_some());
}

#[test]
fn while_a_size_is_forced_a_point_outside_any_viewport_reaches_the_game_one_to_one() {
    let host = TestHost::new();
    let client = SizeRig::picture_holder(&host, "point-forced");
    assert_eq!(SizeRig::game_point((100.0, 2000.0), WINDOW), None);

    SizeRig::set(client.handle(), Some(UHD));

    assert_eq!(SizeRig::game_point((100.0, 2000.0), WINDOW), Some((100.0, 2000.0)));
    assert_eq!(SizeRig::game_point((3839.0, 0.0), WINDOW), Some((3839.0, 0.0)));
}

#[test]
fn while_a_size_is_forced_a_point_in_the_viewport_maps_to_the_full_window_in_back_buffer_pixels() {
    let host = TestHost::new();
    let client = SizeRig::picture_holder(&host, "point-viewport-forced");
    SizeRig::set_viewport(client.handle(), [960.0, 540.0, 2880.0, 1620.0]);
    SizeRig::set(client.handle(), Some(UHD));

    assert_eq!(SizeRig::game_point((1920.0, 1080.0), WINDOW), Some((1920.0, 1080.0)));
    assert_eq!(SizeRig::game_point((960.0, 540.0), WINDOW), Some((0.0, 0.0)));
}

#[test]
fn with_no_size_forced_a_point_in_the_viewport_reaches_the_game_in_client_pixels() {
    let host = TestHost::new();
    let client = SizeRig::picture_holder(&host, "point-viewport");
    SizeRig::set_viewport(client.handle(), [960.0, 540.0, 2880.0, 1620.0]);

    assert_eq!(SizeRig::game_point((1920.0, 1080.0), WINDOW), Some((800.0, 450.0)));
    assert_eq!(SizeRig::game_point((100.0, 100.0), WINDOW), None);
}

#[test]
fn shutdown_while_forced_gives_the_client_size_and_ends_the_size() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-shutdown");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    assert_eq!(SizeRig::shutdown(WINDOW), Some(WINDOW));

    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
    assert_eq!(SizeRig::shutdown(WINDOW), None);
}

#[test]
fn shutdown_with_a_restore_due_gives_the_client_size() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-shutdown-due");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);
    SizeRig::set(client.handle(), None);

    assert_eq!(SizeRig::shutdown(SMALL), Some(SMALL));
    assert_eq!(SizeRig::message(&host.frame_sized(2, SMALL, SMALL)), None);
}

#[test]
fn shutdown_with_nothing_forced_or_a_minimised_window_gives_nothing() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-shutdown-none");
    assert_eq!(SizeRig::shutdown(WINDOW), None);

    SizeRig::set(client.handle(), Some(UHD));

    assert_eq!(SizeRig::shutdown((0, 0)), None);
}

#[test]
fn a_window_resize_while_forced_gives_the_game_the_forced_size_and_a_minimise_passes() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-window");
    assert_eq!(SizeRig::window(false, SMALL), (false, SMALL));

    SizeRig::set(client.handle(), Some(UHD));

    assert_eq!(SizeRig::window(false, SMALL), (true, (3840, 2160)));
    assert_eq!(SizeRig::window(true, (0, 0)), (false, (0, 0)));
}

#[test]
fn disabling_the_holder_restores_the_size() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-disable");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    let index = ModIndex(client.handle().slot as u8);
    host.with_runtime(|runtime| runtime.manager_mut().set_enabled(index, false));
    let restored = host.frame_sized(2, (3840, 2160), WINDOW);

    assert_eq!(SizeRig::message(&restored), Some(WINDOW));
    assert_eq!(SizeRig::resize(WINDOW), (false, WINDOW));
}

#[test]
fn set_render_size_after_the_shutdown_call_is_not_permitted() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-closed");
    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::Ok);
    host.frame_sized(1, WINDOW, WINDOW);
    assert_eq!(SizeRig::shutdown(WINDOW), Some(WINDOW));

    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::NotPermitted);

    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
    assert_eq!(SizeRig::set(client.handle(), None), AmmStatus::Ok);
    assert_eq!(SizeRig::resize(SMALL), (false, SMALL));
}

#[test]
fn a_kept_host_forces_a_size_again_after_the_host_is_created_a_second_time() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-reopen");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);
    assert_eq!(SizeRig::shutdown(WINDOW), Some(WINDOW));
    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::NotPermitted);

    let (config, game, version) =
        (host.config_dir().display().to_string(), host.game_dir().display().to_string(), "1.0.0-test");
    // SAFETY: the three texts live for the call
    let status = unsafe {
        amm_host_create(config.as_ptr(), config.len(), game.as_ptr(), game.len(), version.as_ptr(), version.len())
    };
    assert_eq!(status, AmmStatus::Ok);

    assert_eq!(SizeRig::set(client.handle(), Some(UHD)), AmmStatus::Ok);
    assert_eq!(SizeRig::message(&host.frame_sized(2, WINDOW, WINDOW)), Some((3840, 2160)));
}

#[test]
fn a_window_change_while_set_sends_the_forced_size_on_the_next_frame() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-window-change");
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);
    assert_eq!(SizeRig::message(&host.frame_sized(2, (3840, 2160), WINDOW)), None);

    amm_host_window_changed();

    assert_eq!(SizeRig::message(&host.frame_sized(3, (3840, 2160), WINDOW)), Some((3840, 2160)));
    assert_eq!(SizeRig::message(&host.frame_sized(4, (3840, 2160), WINDOW)), None);
}

#[test]
fn a_window_change_with_nothing_forced_sends_nothing() {
    let host = TestHost::new();
    let _client = SizeRig::holder(&host, "size-window-change-idle");

    amm_host_window_changed();

    assert_eq!(SizeRig::message(&host.frame_sized(1, WINDOW, WINDOW)), None);
    assert_eq!(SizeRig::message(&host.frame_sized(2, WINDOW, WINDOW)), None);
}

#[test]
fn a_window_change_before_a_size_is_set_is_not_kept_for_later() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-window-change-early");
    amm_host_window_changed();
    SizeRig::set(client.handle(), Some(UHD));
    host.frame_sized(1, WINDOW, WINDOW);

    assert_eq!(SizeRig::message(&host.frame_sized(2, (3840, 2160), WINDOW)), None);
}

#[test]
fn while_set_the_forced_size_is_sent_again_after_the_keep_alive() {
    let host = TestHost::new();
    let client = SizeRig::holder(&host, "size-keep-alive");
    SizeRig::set(client.handle(), Some(UHD));
    assert_eq!(SizeRig::message(&host.frame_sized(1000, WINDOW, WINDOW)), Some((3840, 2160)));

    let early = host.frame_sized(1000 + RenderSizeBook::KEEP_ALIVE_MS - 1, (3840, 2160), WINDOW);
    let due = host.frame_sized(1000 + RenderSizeBook::KEEP_ALIVE_MS, (3840, 2160), WINDOW);

    assert_eq!(SizeRig::message(&early), None);
    assert_eq!(SizeRig::message(&due), Some((3840, 2160)));
}
