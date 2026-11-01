use amm_abi::types::AmmSamplerKind;
use amm_overlay::LayerRenderer;

use super::support::{PixelCheck, TestFrame, TestMesh, WarpTarget};

const BLACK: [u8; 4] = [0, 0, 0, 255];
const RED: [u8; 4] = [255, 0, 0, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const TEXTURE: u64 = 1;

#[test]
fn white_texture_quad_draws_its_colour() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut mesh = TestMesh::default();
    mesh.full(TEXTURE, 2, 2, WHITE).quad([8.0, 8.0, 24.0, 24.0], RED, TEXTURE, TestMesh::FULL_CLIP);
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLACK);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    assert_eq!(report.drawn, 1);
    assert_eq!(target.pixel(16, 16), RED);
    assert_eq!(target.pixel(8, 8), RED);
    assert_eq!(target.pixel(40, 40), BLACK);
}

#[test]
fn scissor_rect_clips_the_quad() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut mesh = TestMesh::default();
    mesh.full(TEXTURE, 2, 2, WHITE).quad([0.0, 0.0, 64.0, 64.0], RED, TEXTURE, [0.0, 0.0, 31.5, 64.0]);
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLACK);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    // the right edge 31.5 is rounded up, so column 31 is inside and column 32 is not
    assert_eq!(target.pixel(31, 32), RED);
    assert_eq!(target.pixel(32, 32), BLACK);
    assert_eq!(target.pixel(60, 10), BLACK);
}

#[test]
fn premultiplied_colour_blends_over_the_background() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut mesh = TestMesh::default();
    mesh.full(TEXTURE, 2, 2, WHITE).quad([0.0, 0.0, 64.0, 64.0], [128, 0, 0, 128], TEXTURE, TestMesh::FULL_CLIP);
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear([0, 0, 255, 255]);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    // colour: src * 1 + dst * (1 - 128/255); alpha: src * (1 - dst alpha) + dst * 1
    let pixel = target.pixel(20, 20);
    assert!(PixelCheck::near(pixel, [128, 0, 127, 255], 1), "{pixel:?}");
}

#[test]
fn unknown_texture_shader_or_sampler_skips_only_that_command() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let linear = AmmSamplerKind::Linear as u8;
    let mut mesh = TestMesh::default();
    mesh.full(TEXTURE, 2, 2, WHITE)
        .quad_with([0.0, 0.0, 16.0, 16.0], RED, 99, TestMesh::FULL_CLIP, 0, linear)
        .quad_with([16.0, 0.0, 32.0, 16.0], RED, TEXTURE, TestMesh::FULL_CLIP, 7, linear)
        .quad_with([32.0, 0.0, 48.0, 16.0], RED, TEXTURE, TestMesh::FULL_CLIP, 0, 9)
        .quad_with([48.0, 0.0, 64.0, 16.0], RED, TEXTURE, TestMesh::FULL_CLIP, 0, linear);
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLACK);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    assert_eq!(report.drawn, 1);
    assert_eq!(report.skipped_texture, 1);
    assert_eq!(report.skipped_pipeline, 2);
    assert_eq!(target.pixel(8, 8), BLACK);
    assert_eq!(target.pixel(24, 8), BLACK);
    assert_eq!(target.pixel(40, 8), BLACK);
    assert_eq!(target.pixel(56, 8), RED);
}

#[test]
fn texture_ids_of_one_owner_do_not_reach_another() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mut owner_a = TestMesh::default();
    owner_a.full(TEXTURE, 2, 2, WHITE);
    let mut owner_b = TestMesh::default();
    owner_b.quad([0.0, 0.0, 64.0, 64.0], RED, TEXTURE, TestMesh::FULL_CLIP);
    let layers = TestFrame::new(&[(0, &owner_a), (1, &owner_b)]);
    target.clear(BLACK);

    // SAFETY: the frame points into `layers` and the meshes, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..2) };

    assert_eq!(report.skipped_texture, 1);
    assert_eq!(target.pixel(32, 32), BLACK);
}
