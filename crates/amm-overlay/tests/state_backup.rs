use amm_overlay::{LayerRenderer, StateBackup};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BLEND_DESC, D3D11_BLEND_ONE, D3D11_BLEND_OP_ADD, D3D11_BLEND_ZERO, D3D11_COLOR_WRITE_ENABLE_ALL,
    D3D11_VIEWPORT, ID3D11BlendState, ID3D11RenderTargetView,
};
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_R8G8B8A8_UNORM;
use windows::core::BOOL;

use super::support::{TestFrame, TestMesh, WarpTarget};

const WHITE: [u8; 4] = [255, 255, 255, 255];

#[test]
fn backup_restores_viewport_blend_state_and_render_target() {
    let target = WarpTarget::new();
    let mut renderer = LayerRenderer::new(&target.device).unwrap();
    let game_texture = WarpTarget::texture(&target.device, 32, 32, DXGI_FORMAT_R8G8B8A8_UNORM);
    let game_view = WarpTarget::render_view(&target.device, &game_texture);
    let mut desc = D3D11_BLEND_DESC::default();
    desc.RenderTarget[0].BlendEnable = BOOL::from(true);
    desc.RenderTarget[0].SrcBlend = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlend = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].BlendOp = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].SrcBlendAlpha = D3D11_BLEND_ONE;
    desc.RenderTarget[0].DestBlendAlpha = D3D11_BLEND_ZERO;
    desc.RenderTarget[0].BlendOpAlpha = D3D11_BLEND_OP_ADD;
    desc.RenderTarget[0].RenderTargetWriteMask = D3D11_COLOR_WRITE_ENABLE_ALL.0 as u8;
    let mut game_blend: Option<ID3D11BlendState> = None;
    // SAFETY: desc lives for the call
    unsafe { target.device.CreateBlendState(&desc, Some(&mut game_blend)) }.unwrap();
    let game_blend = game_blend.unwrap();
    let game_viewport =
        D3D11_VIEWPORT { TopLeftX: 1.0, TopLeftY: 2.0, Width: 30.0, Height: 20.0, MinDepth: 0.0, MaxDepth: 1.0 };
    let game_factor = [0.25f32, 0.5, 0.75, 1.0];
    // SAFETY: every object set is live
    unsafe {
        target.ctx.OMSetRenderTargets(Some(&[Some(game_view.clone())]), None);
        target.ctx.RSSetViewports(Some(&[game_viewport]));
        target.ctx.OMSetBlendState(&game_blend, Some(&game_factor), 0x00ff_00ff);
    }
    let mut mesh = TestMesh::default();
    mesh.full(1, 2, 2, WHITE).quad([0.0, 0.0, 64.0, 64.0], WHITE, 1, TestMesh::FULL_CLIP);
    let layers = TestFrame::new(&[(0, &mesh)]);

    {
        let _backup = StateBackup::capture(&target.ctx);
        // SAFETY: the frame points into `layers` and `mesh`, which live for the call
        let report = unsafe { renderer.draw(&target.ctx, &target.view, target.size(), &layers.frame(), 0..1) };
        assert_eq!(report.drawn, 1);
    }

    let mut views: [Option<ID3D11RenderTargetView>; 1] = [None];
    let mut viewport_count = 1u32;
    let mut viewports = [D3D11_VIEWPORT::default()];
    let mut blend: Option<ID3D11BlendState> = None;
    let mut factor = [0.0f32; 4];
    let mut mask = 0u32;
    // SAFETY: every out pointer is a writable local of the matching size
    unsafe {
        target.ctx.OMGetRenderTargets(Some(&mut views), None);
        target.ctx.RSGetViewports(&mut viewport_count, Some(viewports.as_mut_ptr()));
        target.ctx.OMGetBlendState(Some(&mut blend), Some(&mut factor), Some(&mut mask));
    }
    assert_eq!(views[0].as_ref(), Some(&game_view));
    assert_eq!(viewport_count, 1);
    assert_eq!(viewports[0], game_viewport);
    assert_eq!(blend.as_ref(), Some(&game_blend));
    assert_eq!(factor, game_factor);
    assert_eq!(mask, 0x00ff_00ff);
}
