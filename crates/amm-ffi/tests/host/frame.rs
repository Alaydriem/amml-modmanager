use amm_abi::types::{AmmCapability, AmmCursorShape};

use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const KEY_F10: u32 = 0x79;

fn layer_cmds(frame: &amm_ffi::types::AmmHostFrame, owner: u8) -> Vec<[f32; 4]> {
    let layer = TestHost::layers(frame).iter().find(|layer| layer.owner == owner).unwrap_or_else(|| panic!("layer"));
    if layer.mesh.cmd_count == 0 {
        return Vec::new();
    }
    // SAFETY: the host keeps the layer's commands valid until the next frame
    let cmds = unsafe { std::slice::from_raw_parts(layer.mesh.cmds, layer.mesh.cmd_count) };
    cmds.iter().map(|cmd| cmd.clip).collect()
}

#[test]
fn no_clients_and_closed_panel_returns_no_layers() {
    let host = TestHost::new();

    let frame = host.frame(0);

    assert_eq!(frame.layer_count, 0);
    assert!(!frame.cursor_free);
    assert!(!frame.hide_vanilla_hud);
}

#[test]
fn a_client_without_a_layer_lease_is_not_called() {
    let host = TestHost::new();
    let idle = TestClient::register(&host, &Descriptors::overlay("idle"), TestMesh::triangle());

    let frame = host.frame(0);

    assert_eq!(frame.layer_count, 0);
    assert_eq!(idle.frames(), 0);
}

#[test]
fn layers_follow_snapshot_order() {
    let host = TestHost::new();
    let claims = [("Overlay", "Shared"), ("Input", "Shared")];
    let input_holder =
        TestClient::register(&host, &Descriptors::with_claims("pilot", &claims, ""), TestMesh::triangle());
    let overlay = TestClient::register(&host, &Descriptors::overlay("legend"), TestMesh::triangle());
    let hud = TestClient::register(
        &host,
        &Descriptors::with_claims("corner", &[("Hud", "Shared")], ""),
        TestMesh::triangle(),
    );
    input_holder.acquire(AmmCapability::Overlay);
    input_holder.acquire(AmmCapability::Input);
    overlay.acquire(AmmCapability::Overlay);
    hud.acquire(AmmCapability::Hud);

    let frame = host.frame(0);

    assert_eq!(TestHost::owners(&frame), vec![hud.owner(), overlay.owner(), input_holder.owner()]);
    assert_eq!(hud.state().allowed.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(overlay.state().allowed.load(std::sync::atomic::Ordering::SeqCst), 2);
}

#[test]
fn the_frame_callback_runs_once_per_layer_with_one_allowed_bit() {
    let host = TestHost::new();
    let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
    let both = TestClient::register(&host, &Descriptors::with_claims("both", &claims, ""), TestMesh::triangle());
    both.acquire(AmmCapability::Hud);
    both.acquire(AmmCapability::Overlay);

    let frame = host.frame(0);

    let layers = TestHost::layers(&frame);
    assert_eq!(both.calls(), vec![1, 2]);
    assert_eq!(TestHost::owners(&frame), vec![both.owner(), both.owner()]);
    assert_eq!((layers[0].mesh.vertex_count, layers[1].mesh.vertex_count), (3, 3));
    assert_ne!(layers[0].mesh.vertices, layers[1].mesh.vertices);
}

#[test]
fn toggle_chord_opens_the_panel_and_the_next_frame_has_a_manager_layer() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("under"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);

    let opened = host.key(KEY_F10, true, TestHost::CTRL | TestHost::SHIFT, 0);
    host.key(KEY_F10, false, TestHost::CTRL | TestHost::SHIFT, 1);
    let open_frame = host.frame(2);
    let open_owners = TestHost::owners(&open_frame);
    let open_cursor = open_frame.cursor_free;
    let closed = host.key(KEY_F10, true, TestHost::CTRL | TestHost::SHIFT, 3);
    host.key(KEY_F10, false, TestHost::CTRL | TestHost::SHIFT, 4);
    let closed_frame = host.frame(5);

    assert!(opened.consume);
    assert_eq!(open_owners, vec![client.owner(), TestHost::MANAGER]);
    assert!(open_cursor);
    assert!(closed.consume);
    assert_eq!(TestHost::owners(&closed_frame), vec![client.owner()]);
    assert!(!closed_frame.cursor_free);
}

#[test]
fn clip_rects_are_clamped_to_the_back_buffer_rounded_and_empty_ones_skipped() {
    let host = TestHost::new();
    let cmds = vec![
        TestMesh::cmd([-10.0, -10.0, 5000.0, 5000.0]),
        TestMesh::cmd([f32::NAN, 0.0, 10.0, 10.0]),
        TestMesh::cmd([100.0, 100.0, 100.0, 200.0]),
        TestMesh::cmd([3000.0, 3000.0, 4000.0, 4000.0]),
        TestMesh::cmd([10.4, 10.6, 20.5, 30.2]),
    ];
    let client = TestClient::register(&host, &Descriptors::overlay("clipper"), TestMesh::with_cmds(cmds));
    client.acquire(AmmCapability::Overlay);

    let frame = host.frame(0);

    let full = [0.0, 0.0, TestHost::WIDTH as f32, TestHost::HEIGHT as f32];
    assert_eq!(layer_cmds(&frame, client.owner()), vec![full, [10.0, 11.0, 21.0, 30.0]]);
}

#[test]
fn mesh_over_the_vertex_limit_skips_the_frame_without_a_failure() {
    let host = TestHost::new();
    let heavy = TestClient::register(&host, &Descriptors::overlay("heavy"), TestMesh::with_vertices(262_145));
    let light = TestClient::register(&host, &Descriptors::overlay("light"), TestMesh::with_vertices(262_144));
    heavy.acquire(AmmCapability::Overlay);
    light.acquire(AmmCapability::Overlay);

    let mut heavy_drawn = false;
    for now in 0..35 {
        let frame = host.frame(now);
        let layers = TestHost::layers(&frame);
        let owners = TestHost::owners(&frame);
        heavy_drawn |= layers.iter().any(|layer| layer.owner == heavy.owner() && layer.mesh.vertex_count > 0);
        assert!(owners.contains(&light.owner()));
    }

    assert!(!heavy_drawn);
    assert_eq!(heavy.frames(), 35);
}

#[test]
fn texture_over_the_budget_is_dropped_from_the_layer_but_the_mesh_still_draws() {
    let host = TestHost::new();
    let side = 4097;
    let client = TestClient::register(&host, &Descriptors::overlay("huge"), TestMesh::with_texture(1, side));
    client.acquire(AmmCapability::Overlay);

    let frame = host.frame(0);

    let layer = TestHost::layers(&frame).iter().find(|layer| layer.owner == client.owner()).expect("layer");
    assert_eq!(layer.mesh.delta_count, 0);
    assert_eq!(layer.mesh.vertex_count, 3);
}

#[test]
fn a_vertex_capped_frame_still_uploads_and_frees_textures() {
    let host = TestHost::new();
    let mut mesh = TestMesh::with_texture(7, 2);
    mesh.vertices = vec![TestMesh::VERTEX; 262_145];
    mesh.freed = vec![9];
    let client = TestClient::register(&host, &Descriptors::overlay("capped"), mesh);
    client.acquire(AmmCapability::Overlay);

    let frame = host.frame(0);

    let layer = TestHost::layers(&frame).iter().find(|layer| layer.owner == client.owner()).expect("upload layer");
    assert_eq!((layer.mesh.vertex_count, layer.mesh.index_count, layer.mesh.cmd_count), (0, 0, 0));
    assert_eq!((layer.mesh.delta_count, layer.mesh.freed_count), (1, 1));
    // SAFETY: the host keeps the layer memory valid until the next frame
    let freed = unsafe { *layer.mesh.freed };
    assert_eq!(freed, 9);
}

#[test]
fn menu_button_is_hidden_on_other_screens_and_shown_on_the_start_and_pause_screens() {
    let host = TestHost::new();

    let play = TestHost::owners(&host.frame_with(0, TestHost::PLAY));
    let other = TestHost::owners(&host.frame_with(0, 7));
    let pause = TestHost::owners(&host.frame_with(1, TestHost::PAUSE));
    let menu = TestHost::owners(&host.frame_with(1, TestHost::START));
    let menu_rect = TestHost::menu_button_rect();
    let back_in_play = TestHost::owners(&host.frame_with(2, TestHost::PLAY));
    let play_rect = TestHost::menu_button_rect();

    assert!(play.is_empty());
    assert!(other.is_empty());
    assert_eq!(pause, vec![TestHost::MANAGER]);
    assert_eq!(menu, vec![TestHost::MANAGER]);
    let (width, height) = (TestHost::WIDTH as f32, TestHost::HEIGHT as f32);
    let bottom_right = |[min_x, min_y, max_x, max_y]: [f32; 4]| {
        min_x > width / 2.0 && min_y > height / 2.0 && max_x <= width && max_y <= height
    };
    assert!(menu_rect.is_some_and(bottom_right), "the button sits in the bottom-right corner: {menu_rect:?}");
    assert!(back_in_play.is_empty());
    assert_eq!(play_rect, None);
}

#[test]
fn menu_button_is_hidden_while_the_panel_is_open_and_when_disabled_in_config() {
    let host = TestHost::new();
    host.key(KEY_F10, true, TestHost::CTRL | TestHost::SHIFT, 0);
    host.key(KEY_F10, false, TestHost::CTRL | TestHost::SHIFT, 0);
    let open = TestHost::owners(&host.frame_with(1, TestHost::START));
    let open_rect = TestHost::menu_button_rect();
    drop(host);

    let host = TestHost::with_config(
        "menu_button = false
",
    );
    let disabled = host.frame_with(0, TestHost::START).layer_count;
    let disabled_rect = TestHost::menu_button_rect();

    assert_eq!(open, vec![TestHost::MANAGER]);
    assert_eq!(open_rect, None);
    assert_eq!(disabled, 0);
    assert_eq!(disabled_rect, None);
}

#[test]
fn menu_button_hides_under_a_full_window_overlay_and_stays_under_a_small_one() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("library"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let (width, height) = (TestHost::WIDTH as f32, TestHost::HEIGHT as f32);

    client.reply_for_layer(1, (false, false, AmmCursorShape::Default as u8, [0.0, 0.0, 300.0, height]));
    host.frame_with(0, TestHost::PAUSE);
    let rail = TestHost::menu_button_rect();
    client.reply_for_layer(1, (false, false, AmmCursorShape::Default as u8, [0.0, 0.0, width, height]));
    host.frame_with(1, TestHost::PAUSE);
    host.frame_with(2, TestHost::PAUSE);
    let covered = TestHost::menu_button_rect();

    assert!(rail.is_some(), "a small overlay keeps the row");
    assert_eq!(covered, None, "a full-window overlay hides the row");
}

#[test]
fn menu_button_returns_after_a_full_window_overlay_is_released() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("library"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);
    let (width, height) = (TestHost::WIDTH as f32, TestHost::HEIGHT as f32);
    client.reply_for_layer(1, (false, false, AmmCursorShape::Default as u8, [0.0, 0.0, width, height]));
    host.frame_with(0, TestHost::PAUSE);
    host.frame_with(1, TestHost::PAUSE);
    assert_eq!(TestHost::menu_button_rect(), None);

    client.release_raw(AmmCapability::Overlay as u8);
    host.frame_with(2, TestHost::PAUSE);
    host.frame_with(3, TestHost::PAUSE);

    assert!(TestHost::menu_button_rect().is_some(), "the row is back once the overlay is gone");
}

#[test]
fn the_manager_layer_is_always_the_last_layer_of_the_frame() {
    let host = TestHost::new();
    let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
    let first = TestClient::register(&host, &Descriptors::with_claims("first", &claims, ""), TestMesh::triangle());
    let second = TestClient::register(&host, &Descriptors::with_claims("second", &claims, ""), TestMesh::triangle());
    for client in [&first, &second] {
        client.acquire(AmmCapability::Hud);
        client.acquire(AmmCapability::Overlay);
    }

    host.key(KEY_F10, true, TestHost::CTRL | TestHost::SHIFT, 0);
    host.key(KEY_F10, false, TestHost::CTRL | TestHost::SHIFT, 1);
    let open: Vec<Vec<u8>> = (2..5).map(|now_ms| TestHost::owners(&host.frame(now_ms))).collect();

    for owners in &open {
        assert_eq!(owners.len(), 5);
        assert_eq!(owners.iter().position(|owner| *owner == TestHost::MANAGER), Some(owners.len() - 1));
    }
}
