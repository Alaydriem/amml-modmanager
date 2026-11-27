use amm_abi::types::AmmSamplerKind;
use amm_ffi::types::AMM_HOST_GAME_PICTURE_TEXTURE;
use amm_overlay::{GamePicture, LayerRenderer};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_R32_UINT, DXGI_FORMAT_UNKNOWN,
};

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
    let picture = renderer.game_picture_mut();
    let texture = picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap().clone();
    // SAFETY: both textures are live, of one size and format, on one device; this stands for the loader's copy
    unsafe { target.ctx.CopyResource(&texture, &back_buffer) };
    picture.set_copied(true);
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

#[test]
fn a_failed_copy_is_not_drawn() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let picture = renderer.game_picture_mut();
    picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap();
    picture.set_copied(false);
    let mesh = picture_mesh();
    let layers = TestFrame::new(&[(0, &mesh)]);
    target.clear(BLUE);

    // SAFETY: the frame points into `layers` and `mesh`, which live for the call
    let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };

    assert_eq!(report.skipped_texture, 1);
    assert_eq!(target.pixel(32, 32), BLUE);
}

#[test]
fn prepare_keeps_the_texture_for_the_same_surface_and_remakes_it_for_another() {
    let target = WarpTarget::new();
    let mut picture = GamePicture::new();

    let first = picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap().clone();
    picture.set_copied(true);
    let same = picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap().clone();
    assert_eq!(first, same);
    let resized = picture.prepare(&target.device, 32, 64, DXGI_FORMAT_R8G8B8A8_UNORM).unwrap().clone();

    assert_ne!(first, resized);
    // a remade texture holds no copy yet
    assert!(picture.view().is_none());
    assert_eq!(picture.object_count(), 2);
}

#[test]
fn an_srgb_back_buffer_is_pictured_through_its_unorm_twin() {
    let target = WarpTarget::new();
    let mut picture = GamePicture::new();
    let texture = picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB).unwrap().clone();
    let mut desc = Default::default();
    // SAFETY: the texture is live and `desc` is writable for the call
    unsafe { texture.GetDesc(&mut desc) };

    assert_eq!(desc.Format, DXGI_FORMAT_R8G8B8A8_UNORM);
}

#[test]
fn a_surface_that_cannot_be_pictured_makes_no_texture() {
    let target = WarpTarget::new();
    let mut picture = GamePicture::new();

    assert!(picture.prepare(&target.device, 64, 64, DXGI_FORMAT_UNKNOWN).is_err());
    assert!(picture.prepare(&target.device, 64, 64, DXGI_FORMAT_R32_UINT).is_err());
    assert!(picture.prepare(&target.device, 0, 64, DXGI_FORMAT_R8G8B8A8_UNORM).is_err());
    assert_eq!(picture.object_count(), 0);
}
