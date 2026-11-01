use std::path::Path;

use amm_abi::types::AmmHandle;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::registry::ModIndex;
use amm_ffi::services::HostRuntime;
use amm_ffi::types::{AmmHostFrame, AmmHostFrameInput, AmmHostLayer};
use tempfile::TempDir;

use crate::support::{Descriptors, TestClient, TestMesh};

const INPUT: AmmHostFrameInput = AmmHostFrameInput {
    width: 1920,
    height: 1080,
    pixels_per_point: 1.0,
    time_secs: 0.0,
    now_ms: 0,
    menu_screen: 0,
    format: 28,
    client_width: 1920,
    client_height: 1080,
};

/// A runtime driven directly, with the given `budgets` block in its config, and one Overlay mod.
struct Rig {
    runtime: Option<HostRuntime>,
    handle: AmmHandle,
    state: std::sync::Arc<crate::support::test_client::ClientState>,
    _dir: TempDir,
}

impl Rig {
    fn new(budgets: &str, mesh: TestMesh) -> Rig {
        Self::with_layers(budgets, mesh, &[Capability::Overlay])
    }

    /// A rig whose mod claims and acquires each of `layers` (Hud and Overlay).
    fn with_layers(budgets: &str, mesh: TestMesh, layers: &[Capability]) -> Rig {
        Self::with_descriptor(budgets, mesh, layers, "")
    }

    /// A rig whose descriptor has `extra` HCL after its claims.
    fn with_descriptor(budgets: &str, mesh: TestMesh, layers: &[Capability], extra: &str) -> Rig {
        let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("temp dir: {error:?}"));
        let manager_dir = dir.path().join("manager");
        std::fs::create_dir_all(&manager_dir).unwrap_or_else(|error| panic!("manager folder: {error:?}"));
        std::fs::write(manager_dir.join("config.hcl"), format!("budgets {{\n{budgets}\n}}\n"))
            .unwrap_or_else(|error| panic!("config: {error:?}"));
        let mut runtime = HostRuntime::new(manager_dir, dir.path().join("game"), "test".into())
            .unwrap_or_else(|error| panic!("runtime: {error:?}"));
        let (state, callbacks) = TestClient::detached(mesh);
        let mod_dir = dir.path().join("mods").join("rig").join("config");
        let claims: Vec<(&str, &str)> = layers
            .iter()
            .map(|capability| (if *capability == Capability::Hud { "Hud" } else { "Overlay" }, "Shared"))
            .collect();
        let descriptor = Descriptors::with_claims("rig", &claims, extra);
        let handle = runtime
            .register(descriptor.as_bytes(), Path::new(&mod_dir), callbacks)
            .unwrap_or_else(|error| panic!("registered: {error:?}"));
        for capability in layers {
            runtime.acquire(handle, *capability).unwrap_or_else(|error| panic!("layer: {error:?}"));
        }
        Rig { runtime: Some(runtime), handle, state, _dir: dir }
    }

    fn runtime(&mut self) -> &mut HostRuntime {
        self.runtime.as_mut().unwrap_or_else(|| panic!("runtime"))
    }

    fn index(&self) -> ModIndex {
        ModIndex(self.handle.slot as u8)
    }

    fn layer(frame: &AmmHostFrame) -> AmmHostLayer {
        assert_eq!(frame.layer_count, 1);
        // SAFETY: the runtime keeps the layers valid until its next frame
        unsafe { *frame.layers }
    }

    fn layers(frame: &AmmHostFrame) -> Vec<AmmHostLayer> {
        // SAFETY: the runtime keeps the layers valid until its next frame
        unsafe { std::slice::from_raw_parts(frame.layers, frame.layer_count as usize) }.to_vec()
    }
}

impl Drop for Rig {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown();
        }
    }
}

#[test]
fn index_and_command_caps_leave_out_the_draw_only() {
    let mut index_capped = Rig::new("indices_per_mod = 2", TestMesh::with_texture(1, 2));
    let mut two_cmds = TestMesh::with_texture(1, 2);
    two_cmds.cmds.push(TestMesh::cmd([0.0, 0.0, 10.0, 10.0]));
    let mut cmd_capped = Rig::new("draw_cmds_per_mod = 1", two_cmds);

    let by_indices = Rig::layer(&index_capped.runtime().frame(&INPUT));
    let by_cmds = Rig::layer(&cmd_capped.runtime().frame(&INPUT));

    for (rig, layer) in [(&mut index_capped, by_indices), (&mut cmd_capped, by_cmds)] {
        assert_eq!((layer.mesh.vertex_count, layer.mesh.index_count, layer.mesh.cmd_count), (0, 0, 0));
        assert_eq!(layer.mesh.delta_count, 1);
        assert_eq!(rig.runtime().manager().telemetry().vertex_skips(), 1);
    }
}

#[test]
fn upload_bytes_per_frame_are_capped_and_counted() {
    let mut mesh = TestMesh::with_texture(1, 2);
    let first = mesh.deltas[0];
    mesh.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    mesh.deltas.push(amm_abi::types::AmmTextureDelta { id: 3, ..first });
    let mut rig = Rig::new("upload_bytes_per_frame = 40", mesh);

    let layer = Rig::layer(&rig.runtime().frame(&INPUT));

    assert_eq!(layer.mesh.delta_count, 2);
    assert_eq!(layer.mesh.byte_count, 32);
    assert_eq!(rig.runtime().manager().telemetry().texture_refusals(), 1);
}

#[test]
fn freed_ids_per_frame_are_capped() {
    let mut mesh = TestMesh::triangle();
    mesh.freed = vec![1, 2, 3];
    let mut rig = Rig::new("freed_ids_per_frame = 2", mesh);

    let layer = Rig::layer(&rig.runtime().frame(&INPUT));

    assert_eq!(layer.mesh.freed_count, 2);
    assert_eq!(rig.runtime().manager().telemetry().freed_drops(), 1);
}

#[test]
fn unregister_keeps_the_copy_buffers_until_the_next_frame_then_releases_them() {
    let mut rig = Rig::new("", TestMesh::with_texture(1, 8));
    let index = rig.index();
    let handle = rig.handle;
    let layer = Rig::layer(&rig.runtime().frame(&INPUT));
    let held = rig.runtime().manager().telemetry().retained_bytes(index);

    rig.runtime().unregister(handle).expect("unregistered");
    let after_unregister = rig.runtime().manager().telemetry().retained_bytes(index);
    let next = rig.runtime().frame(&INPUT);

    assert!(held >= 256);
    assert!(!layer.mesh.vertices.is_null());
    assert_eq!(after_unregister, held);
    assert_eq!(next.layer_count, 0);
    assert_eq!(rig.runtime().manager().telemetry().retained_bytes(index), 0);
}

#[test]
fn out_of_range_indices_are_a_frame_failure() {
    let mut mesh = TestMesh::triangle();
    mesh.indices = vec![0, 1, 3];
    let mut bad = Rig::new("", mesh);
    let mut good = Rig::new("", TestMesh::triangle());

    let bad_layers = bad.runtime().frame(&INPUT).layer_count;
    let good_layers = good.runtime().frame(&INPUT).layer_count;

    assert_eq!(bad_layers, 0);
    assert_eq!(good_layers, 1);
}

#[test]
fn texture_deltas_per_frame_are_capped_and_counted() {
    let mut mesh = TestMesh::with_texture(1, 2);
    let first = mesh.deltas[0];
    mesh.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    mesh.deltas.push(amm_abi::types::AmmTextureDelta { id: 3, ..first });
    let mut rig = Rig::new("texture_deltas_per_frame = 2", mesh);

    let layer = Rig::layer(&rig.runtime().frame(&INPUT));

    assert_eq!(layer.mesh.delta_count, 2);
    assert_eq!(rig.runtime().manager().telemetry().texture_refusals(), 1);
}

#[test]
fn textures_per_mod_are_capped_and_counted() {
    let mut mesh = TestMesh::with_texture(1, 2);
    let first = mesh.deltas[0];
    mesh.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    let mut rig = Rig::new("textures_per_mod = 1", mesh);

    let layer = Rig::layer(&rig.runtime().frame(&INPUT));

    assert_eq!(layer.mesh.delta_count, 1);
    assert_eq!(rig.runtime().manager().telemetry().texture_refusals(), 1);
}

#[test]
fn the_host_refuses_a_delta_past_the_granted_budget_and_accepts_it_under() {
    let mut three = TestMesh::with_texture(1, 2);
    let first = three.deltas[0];
    three.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    three.deltas.push(amm_abi::types::AmmTextureDelta { id: 3, ..first });
    let mut two_mib = TestMesh::with_texture(1, 512);
    let first = two_mib.deltas[0];
    two_mib.deltas.push(amm_abi::types::AmmTextureDelta { id: 2, ..first });
    let overlay = &[Capability::Overlay];
    let mut by_count = Rig::with_descriptor("textures_per_mod = 1", three, overlay, "texture_budget_count = 2\n");
    let mut by_bytes = Rig::with_descriptor("", two_mib, overlay, "texture_budget_mb = 1\n");

    let counted = Rig::layer(&by_count.runtime().frame(&INPUT));
    let sized = Rig::layer(&by_bytes.runtime().frame(&INPUT));

    assert_eq!(counted.mesh.delta_count, 2);
    assert_eq!(by_count.runtime().manager().telemetry().texture_refusals(), 1);
    assert_eq!(sized.mesh.delta_count, 1);
    assert_eq!(by_bytes.runtime().manager().telemetry().texture_refusals(), 1);
}

#[test]
fn the_vertex_budget_counts_both_layers_of_a_mod() {
    let mut rig =
        Rig::with_layers("vertices_per_mod = 5", TestMesh::triangle(), &[Capability::Hud, Capability::Overlay]);

    let layers = Rig::layers(&rig.runtime().frame(&INPUT));

    let counts: Vec<usize> = layers.iter().map(|layer| layer.mesh.vertex_count).collect();
    assert_eq!(counts, vec![3, 0]);
    assert_eq!(rig.runtime().manager().telemetry().vertex_skips(), 1);
}

#[test]
fn the_upload_budget_counts_both_layers_of_a_mod() {
    let mut rig = Rig::with_layers(
        "upload_bytes_per_frame = 20",
        TestMesh::with_texture(1, 2),
        &[Capability::Hud, Capability::Overlay],
    );

    let layers = Rig::layers(&rig.runtime().frame(&INPUT));

    let deltas: Vec<usize> = layers.iter().map(|layer| layer.mesh.delta_count).collect();
    assert_eq!(deltas, vec![1, 0]);
    assert_eq!(rig.runtime().manager().telemetry().texture_refusals(), 1);
}

#[test]
fn a_mod_whose_two_layers_fail_counts_one_failure_per_frame() {
    let mut rig =
        Rig::with_layers("frame_failure_limit = 4", TestMesh::triangle(), &[Capability::Hud, Capability::Overlay]);
    rig.state.status.store(amm_abi::types::AmmStatus::InvalidArgument as i32, std::sync::atomic::Ordering::SeqCst);

    for _ in 0..6 {
        rig.runtime().frame(&INPUT);
    }

    // three frames call both layers; the Hud layer of the fourth reaches the limit and the mod is no longer called
    assert_eq!(rig.state.frames.load(std::sync::atomic::Ordering::SeqCst), 7);
}

#[test]
fn a_mod_whose_overlay_layer_always_fails_is_disabled_after_the_limit_even_when_its_hud_layer_succeeds() {
    let mut rig =
        Rig::with_layers("frame_failure_limit = 4", TestMesh::triangle(), &[Capability::Hud, Capability::Overlay]);
    rig.state.fail_layers.store(2, std::sync::atomic::Ordering::SeqCst);
    let index = rig.index();

    for _ in 0..6 {
        rig.runtime().frame(&INPUT);
    }

    let failed = rig.runtime().manager().registry().get(index).is_some_and(|registered| registered.failed);
    assert!(failed, "the mod is still drawn");
    assert_eq!(rig.state.frames.load(std::sync::atomic::Ordering::SeqCst), 8);
}

fn game_picture_cmd() -> amm_abi::types::AmmDrawCmd {
    let cmd = TestMesh::cmd([0.0, 0.0, 100.0, 100.0]);
    amm_abi::types::AmmDrawCmd { texture: amm_core::structs::mesh::GamePicture::HOST_TEXTURE, shader: 2, ..cmd }
}

#[test]
fn a_frame_asks_for_the_game_picture_only_when_an_overlay_holder_draws_it() {
    let mut drawing = Rig::new("", TestMesh::with_cmds(vec![game_picture_cmd()]));
    let mut not_drawing = Rig::new("", TestMesh::triangle());

    let drawn = drawing.runtime().frame(&INPUT);
    let layer = Rig::layer(&drawn);
    let plain = not_drawing.runtime().frame(&INPUT);

    assert!(drawn.game_picture);
    assert_eq!(layer.mesh.cmd_count, 1);
    assert!(!plain.game_picture);
}

#[test]
fn a_mod_without_overlay_cannot_draw_the_game_picture() {
    let mesh = TestMesh::with_cmds(vec![game_picture_cmd(), TestMesh::cmd([0.0, 0.0, 10.0, 10.0])]);
    let mut rig = Rig::with_layers("", mesh, &[Capability::Hud]);

    let frame = rig.runtime().frame(&INPUT);
    let layer = Rig::layer(&frame);

    assert!(!frame.game_picture);
    assert_eq!(layer.mesh.cmd_count, 1, "only the game picture command is dropped");
}

#[test]
fn a_game_picture_left_out_by_the_vertex_budget_is_not_asked_for() {
    let mut rig = Rig::new("vertices_per_mod = 2", TestMesh::with_cmds(vec![game_picture_cmd()]));

    let frame = rig.runtime().frame(&INPUT);

    assert!(!frame.game_picture);
}
