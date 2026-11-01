use amm_abi::types::{AmmSamplerKind, AmmTextureDelta};
use amm_ffi::types::AmmHostLayer;
use amm_overlay::{LayerRenderer, TextureRegistry};

use super::support::{TestFrame, TestMesh, WarpTarget};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const GREEN: [u8; 4] = [0, 255, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const TEXTURE: u64 = 1;

/// Draws one frame of `mesh` for owner 0 over a black target and returns the centre pixel.
fn draw_centre(target: &WarpTarget, renderer: &mut LayerRenderer, mesh: &TestMesh) -> [u8; 4] {
    let layers = TestFrame::new(&[(0, mesh)]);
    target.clear(BLACK);
    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };
    target.pixel(32, 32)
}

fn textured_quad() -> TestMesh {
    let mut mesh = TestMesh::default();
    mesh.quad_with([0.0, 0.0, 64.0, 64.0], WHITE, TEXTURE, TestMesh::FULL_CLIP, 0, AmmSamplerKind::Nearest as u8);
    mesh
}

#[test]
fn partial_delta_outside_the_texture_is_rejected_and_the_texture_kept() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut create = textured_quad();
    create.full(TEXTURE, 2, 2, GREEN);
    assert_eq!(draw_centre(&target, &mut renderer, &create), GREEN);

    let mut outside = textured_quad();
    outside.delta(TEXTURE, 0, 1, 1, 2, 2, RED);
    assert_eq!(draw_centre(&target, &mut renderer, &outside), GREEN);

    let mut inside = textured_quad();
    inside.delta(TEXTURE, 0, 0, 0, 2, 2, BLUE);
    assert_eq!(draw_centre(&target, &mut renderer, &inside), BLUE);
}

#[test]
fn delta_whose_pixels_do_not_fit_the_bytes_is_skipped() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut mesh = textured_quad();
    mesh.full(TEXTURE, 2, 2, GREEN);
    mesh.deltas[0].byte_len = 15;

    assert_eq!(draw_centre(&target, &mut renderer, &mesh), BLACK);
    assert!(renderer.textures().is_empty());
}

#[test]
fn listed_freed_ids_leave_after_the_range_is_drawn() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut mesh = textured_quad();
    mesh.full(TEXTURE, 2, 2, GREEN);
    mesh.freed.push(TEXTURE);

    assert_eq!(draw_centre(&target, &mut renderer, &mesh), GREEN);
    assert!(renderer.textures().find(0, TEXTURE).is_none());
}

#[test]
fn freeing_an_owner_bit_drops_only_that_owner() {
    let target = WarpTarget::new();
    let mut registry = TextureRegistry::new();
    let mut mesh = TestMesh::default();
    mesh.full(1, 1, 1, WHITE).full(2, 1, 1, WHITE);
    registry.apply(&target.device, &target.ctx, 0, &mesh.deltas, &mesh.bytes);
    registry.apply(&target.device, &target.ctx, 3, &mesh.deltas, &mesh.bytes);
    assert_eq!(registry.len(), 4);

    registry.free(3, &[2, 77]);
    assert_eq!(registry.len(), 3);
    assert!(registry.find(3, 2).is_none());
    assert!(registry.find(3, 1).is_some());

    registry.free_owners(1 << 0);
    assert_eq!(registry.len(), 1);
    assert!(registry.find(0, 1).is_none());
    assert!(registry.find(3, 1).is_some());

    registry.drop_all();
    assert!(registry.is_empty());
}

#[test]
fn mods_stop_at_the_cap_and_the_manager_keeps_its_reserve() {
    let target = WarpTarget::new();
    let mut registry = TextureRegistry::new();
    let mod_cap = TextureRegistry::MAX_TEXTURES - TextureRegistry::MANAGER_RESERVED;
    let bytes = [255u8; 4];
    let deltas: Vec<AmmTextureDelta> = (0..mod_cap as u64 + 1)
        .map(|id| AmmTextureDelta {
            id,
            sampler: AmmSamplerKind::Linear as u8,
            full: 1,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            byte_offset: 0,
            byte_len: 4,
        })
        .collect();

    registry.apply(&target.device, &target.ctx, 0, &deltas[..mod_cap], &bytes);
    assert_eq!(registry.len(), mod_cap);
    registry.apply(&target.device, &target.ctx, 1, &deltas[mod_cap..], &bytes);
    assert_eq!(registry.len(), mod_cap);
    assert!(registry.find(1, mod_cap as u64).is_none());

    // a full delta for an id the owner already holds replaces it and needs no room
    registry.apply(&target.device, &target.ctx, 0, &deltas[..1], &bytes);
    assert_eq!(registry.len(), mod_cap);

    registry.apply(&target.device, &target.ctx, AmmHostLayer::MANAGER, &deltas[..1], &bytes);
    assert_eq!(registry.len(), mod_cap + 1);
    assert!(registry.find(AmmHostLayer::MANAGER, 0).is_some());
}
