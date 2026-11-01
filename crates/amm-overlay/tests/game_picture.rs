use amm_abi::types::AmmSamplerKind;
use amm_ffi::types::AMM_HOST_GAME_PICTURE_TEXTURE;
use amm_overlay::LayerRenderer;
use windows::Win32::Graphics::Direct3D11::ID3D11Resource;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows::core::Interface;

use super::support::{TestFrame, TestMesh, WarpTarget};

const BLUE: [u8; 4] = [0, 0, 255, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const SHADER_PICTURE: u8 = 2;

/// A layer that draws the game picture over the whole target.
fn picture_mesh() -> TestMesh {
    let mut mesh = TestMesh::default();
    mesh.quad_with(
        [0.0, 0.0, 64.0, 64.0],
        WHITE,
        AMM_HOST_GAME_PICTURE_TEXTURE,
        TestMesh::FULL_CLIP,
        SHADER_PICTURE,
        AmmSamplerKind::Nearest as u8,
    );
    mesh
}

#[test]
fn game_picture_draws_the_copied_back_buffer_opaque() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let back_buffer = WarpTarget::texture(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM);
    let back_view = WarpTarget::render_view(&target.device, &back_buffer);
    // a back buffer alpha of 0 means nothing: the picture is still drawn opaque
    WarpTarget::clear_view(&target.ctx, &back_view, [0, 255, 0, 0]);
    let back_resource: ID3D11Resource = back_buffer.cast().unwrap();
    renderer.game_picture_mut().update(&target.device, &target.ctx, &back_resource);
    let mesh = picture_mesh();
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLUE);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    assert_eq!(report.drawn, 1);
    assert_eq!(target.pixel(32, 32), [0, 255, 0, 255]);
}

#[test]
fn game_picture_before_its_first_copy_is_skipped() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let mesh = picture_mesh();
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLUE);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    assert_eq!(report.skipped_texture, 1);
    assert_eq!(target.pixel(32, 32), BLUE);
}
