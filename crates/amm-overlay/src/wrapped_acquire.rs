use windows::Win32::Graphics::Direct3D11::{ID3D11DeviceContext, ID3D11Resource};
use windows::Win32::Graphics::Direct3D11on12::ID3D11On12Device;

/// A back-buffer wrapper acquired for the D3D11 work of one Present. On drop, also when a panic unwinds through the
/// draw, the target is unbound, the wrapper released and the context flushed, so the D3D12 back buffer is back in
/// the PRESENT state and nothing holds it bound.
pub(crate) struct WrappedAcquire<'a> {
    on12: &'a ID3D11On12Device,
    ctx: &'a ID3D11DeviceContext,
    resources: [Option<ID3D11Resource>; 1],
}

impl<'a> WrappedAcquire<'a> {
    pub(crate) fn new(on12: &'a ID3D11On12Device, ctx: &'a ID3D11DeviceContext, wrapped: &ID3D11Resource) -> Self {
        let resources = [Some(wrapped.clone())];
        // SAFETY: the wrapper is live, and it is released when this value drops, in the same Present
        unsafe { on12.AcquireWrappedResources(&resources) };
        Self { on12, ctx, resources }
    }
}

impl Drop for WrappedAcquire<'_> {
    fn drop(&mut self) {
        // SAFETY: the wrapper was acquired in `new`; binding no target takes no object
        unsafe {
            self.ctx.OMSetRenderTargets(None, None);
            self.on12.ReleaseWrappedResources(&self.resources);
            self.ctx.Flush();
        }
    }
}
