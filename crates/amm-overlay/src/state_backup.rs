use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D::{D3D_PRIMITIVE_TOPOLOGY, D3D_PRIMITIVE_TOPOLOGY_UNDEFINED};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT, D3D11_VIEWPORT, D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE,
    ID3D11BlendState, ID3D11Buffer, ID3D11ClassInstance, ID3D11DepthStencilState, ID3D11DepthStencilView,
    ID3D11DeviceContext, ID3D11DomainShader, ID3D11GeometryShader, ID3D11HullShader, ID3D11InputLayout,
    ID3D11PixelShader, ID3D11RasterizerState, ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView,
    ID3D11VertexShader,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_UNKNOWN};

/// The D3D11 pipeline state a layer draw changes, saved by `capture` and put back when the backup is dropped, so
/// a draw on the game's own context leaves the game's state as it was: the render targets and depth view, the
/// viewports and scissor rectangles, the rasterizer, blend and depth states, pixel shader slot 0 (view and
/// sampler), the vertex, pixel, geometry, hull and domain shaders, vertex constant buffer 0, and the input
/// assembler. Compute, UAVs, stream-out, predication and pixel constant buffers are not saved; the layer draw
/// does not touch them.
pub struct StateBackup {
    ctx: ID3D11DeviceContext,
    targets: [Option<ID3D11RenderTargetView>; Self::MAX_TARGETS],
    depth_view: Option<ID3D11DepthStencilView>,
    scissor_count: u32,
    scissors: [RECT; Self::MAX_RECTS],
    viewport_count: u32,
    viewports: [D3D11_VIEWPORT; Self::MAX_RECTS],
    raster: Option<ID3D11RasterizerState>,
    blend: Option<ID3D11BlendState>,
    blend_factor: [f32; 4],
    sample_mask: u32,
    depth: Option<ID3D11DepthStencilState>,
    stencil_ref: u32,
    ps_view: [Option<ID3D11ShaderResourceView>; 1],
    ps_sampler: [Option<ID3D11SamplerState>; 1],
    ps: Option<ID3D11PixelShader>,
    vs: Option<ID3D11VertexShader>,
    gs: Option<ID3D11GeometryShader>,
    hs: Option<ID3D11HullShader>,
    ds: Option<ID3D11DomainShader>,
    ps_instance_count: u32,
    vs_instance_count: u32,
    gs_instance_count: u32,
    ps_instances: [Option<ID3D11ClassInstance>; Self::MAX_CLASS_INSTANCES],
    vs_instances: [Option<ID3D11ClassInstance>; Self::MAX_CLASS_INSTANCES],
    gs_instances: [Option<ID3D11ClassInstance>; Self::MAX_CLASS_INSTANCES],
    vs_constants: [Option<ID3D11Buffer>; 1],
    topology: D3D_PRIMITIVE_TOPOLOGY,
    index_buffer: Option<ID3D11Buffer>,
    index_format: DXGI_FORMAT,
    index_offset: u32,
    vertex_buffer: Option<ID3D11Buffer>,
    vertex_stride: u32,
    vertex_offset: u32,
    layout: Option<ID3D11InputLayout>,
}

impl StateBackup {
    const MAX_CLASS_INSTANCES: usize = 256;
    const MAX_RECTS: usize = D3D11_VIEWPORT_AND_SCISSORRECT_OBJECT_COUNT_PER_PIPELINE as usize;
    const MAX_TARGETS: usize = D3D11_SIMULTANEOUS_RENDER_TARGET_COUNT as usize;

    /// Saves the state of `ctx`.
    pub fn capture(ctx: &ID3D11DeviceContext) -> StateBackup {
        let mut backup = StateBackup {
            ctx: ctx.clone(),
            targets: Default::default(),
            depth_view: None,
            scissor_count: Self::MAX_RECTS as u32,
            scissors: [RECT::default(); Self::MAX_RECTS],
            viewport_count: Self::MAX_RECTS as u32,
            viewports: [D3D11_VIEWPORT::default(); Self::MAX_RECTS],
            raster: None,
            blend: None,
            blend_factor: [0.0; 4],
            sample_mask: 0,
            depth: None,
            stencil_ref: 0,
            ps_view: [None],
            ps_sampler: [None],
            ps: None,
            vs: None,
            gs: None,
            hs: None,
            ds: None,
            ps_instance_count: Self::MAX_CLASS_INSTANCES as u32,
            vs_instance_count: Self::MAX_CLASS_INSTANCES as u32,
            gs_instance_count: Self::MAX_CLASS_INSTANCES as u32,
            ps_instances: std::array::from_fn(|_| None),
            vs_instances: std::array::from_fn(|_| None),
            gs_instances: std::array::from_fn(|_| None),
            vs_constants: [None],
            topology: D3D_PRIMITIVE_TOPOLOGY_UNDEFINED,
            index_buffer: None,
            index_format: DXGI_FORMAT_UNKNOWN,
            index_offset: 0,
            vertex_buffer: None,
            vertex_stride: 0,
            vertex_offset: 0,
            layout: None,
        };
        backup.save();
        backup
    }

    fn save(&mut self) {
        let ctx = &self.ctx;
        // SAFETY: every out pointer is a field of self, writable for the call, and each array holds the count the
        // call is given; each Get adds a reference that the Option fields own
        unsafe {
            ctx.OMGetRenderTargets(Some(&mut self.targets), Some(&mut self.depth_view));
            ctx.RSGetScissorRects(&mut self.scissor_count, Some(self.scissors.as_mut_ptr()));
            ctx.RSGetViewports(&mut self.viewport_count, Some(self.viewports.as_mut_ptr()));
            self.raster = ctx.RSGetState().ok();
            ctx.OMGetBlendState(Some(&mut self.blend), Some(&mut self.blend_factor), Some(&mut self.sample_mask));
            ctx.OMGetDepthStencilState(Some(&mut self.depth), Some(&mut self.stencil_ref));
            ctx.PSGetShaderResources(0, Some(&mut self.ps_view));
            ctx.PSGetSamplers(0, Some(&mut self.ps_sampler));
            ctx.PSGetShader(&mut self.ps, Some(self.ps_instances.as_mut_ptr()), Some(&mut self.ps_instance_count));
            ctx.VSGetShader(&mut self.vs, Some(self.vs_instances.as_mut_ptr()), Some(&mut self.vs_instance_count));
            ctx.GSGetShader(&mut self.gs, Some(self.gs_instances.as_mut_ptr()), Some(&mut self.gs_instance_count));
            ctx.HSGetShader(&mut self.hs, None, None);
            ctx.DSGetShader(&mut self.ds, None, None);
            ctx.VSGetConstantBuffers(0, Some(&mut self.vs_constants));
            self.topology = ctx.IAGetPrimitiveTopology();
            ctx.IAGetIndexBuffer(
                Some(&mut self.index_buffer),
                Some(&mut self.index_format),
                Some(&mut self.index_offset),
            );
            ctx.IAGetVertexBuffers(
                0,
                1,
                Some(&mut self.vertex_buffer),
                Some(&mut self.vertex_stride),
                Some(&mut self.vertex_offset),
            );
            self.layout = ctx.IAGetInputLayout().ok();
        }
    }

    fn instances(instances: &[Option<ID3D11ClassInstance>], count: u32) -> Option<&[Option<ID3D11ClassInstance>]> {
        let count = (count as usize).min(instances.len());
        if count == 0 { None } else { Some(&instances[..count]) }
    }
}

// each Set takes its own reference; the saved ones are released when the fields drop after it
impl Drop for StateBackup {
    fn drop(&mut self) {
        let ctx = &self.ctx;
        let scissors = &self.scissors[..(self.scissor_count as usize).min(Self::MAX_RECTS)];
        let viewports = &self.viewports[..(self.viewport_count as usize).min(Self::MAX_RECTS)];
        // SAFETY: every object passed is live or None, and each slice holds the count the matching Get returned
        unsafe {
            ctx.OMSetRenderTargets(Some(&self.targets), self.depth_view.as_ref());
            ctx.RSSetScissorRects(Some(scissors));
            ctx.RSSetViewports(Some(viewports));
            ctx.RSSetState(self.raster.as_ref());
            ctx.OMSetBlendState(self.blend.as_ref(), Some(&self.blend_factor), self.sample_mask);
            ctx.OMSetDepthStencilState(self.depth.as_ref(), self.stencil_ref);
            ctx.PSSetShaderResources(0, Some(&self.ps_view));
            ctx.PSSetSamplers(0, Some(&self.ps_sampler));
            ctx.PSSetShader(self.ps.as_ref(), Self::instances(&self.ps_instances, self.ps_instance_count));
            ctx.VSSetShader(self.vs.as_ref(), Self::instances(&self.vs_instances, self.vs_instance_count));
            ctx.GSSetShader(self.gs.as_ref(), Self::instances(&self.gs_instances, self.gs_instance_count));
            ctx.HSSetShader(self.hs.as_ref(), None);
            ctx.DSSetShader(self.ds.as_ref(), None);
            ctx.VSSetConstantBuffers(0, Some(&self.vs_constants));
            ctx.IASetPrimitiveTopology(self.topology);
            ctx.IASetIndexBuffer(self.index_buffer.as_ref(), self.index_format, self.index_offset);
            ctx.IASetVertexBuffers(
                0,
                1,
                Some(&self.vertex_buffer),
                Some(&self.vertex_stride),
                Some(&self.vertex_offset),
            );
            ctx.IASetInputLayout(self.layout.as_ref());
        }
    }
}
