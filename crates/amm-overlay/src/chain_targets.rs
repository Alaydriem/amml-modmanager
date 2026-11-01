use windows::Win32::Foundation::E_POINTER;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, ID3D11RenderTargetView, ID3D11Resource, ID3D11Texture2D,
};
use windows::Win32::Graphics::Direct3D11on12::D3D11_RESOURCE_FLAGS;
use windows::Win32::Graphics::Direct3D12::{D3D12_RESOURCE_STATE_PRESENT, ID3D12Resource};
use windows::Win32::Graphics::Dxgi::{DXGI_MAX_SWAP_CHAIN_BUFFERS, DXGI_SWAP_CHAIN_DESC, IDXGISwapChain};
use windows::core::Interface;

use crate::device::OverlayDevice;

/// The render target views of the bound chain's back buffers. On D3D11 there is one, for buffer 0, which a flip
/// chain always shows as its current back buffer. On D3D12 there is one per buffer, each over a D3D11On12 wrapper
/// of it. Every reference to a back buffer must be gone before the chain's `ResizeBuffers`, or it fails.
pub(crate) struct ChainTargets {
    wrapped: [Option<ID3D11Resource>; Self::MAX_BUFFERS],
    views: [Option<ID3D11RenderTargetView>; Self::MAX_BUFFERS],
    count: usize,
    // identity only: the chain these targets belong to, with no reference held; 0 when there are none
    chain: usize,
    size: (u32, u32),
}

impl ChainTargets {
    const MAX_BUFFERS: usize = DXGI_MAX_SWAP_CHAIN_BUFFERS as usize;

    pub(crate) fn new() -> Self {
        Self { wrapped: Default::default(), views: Default::default(), count: 0, chain: 0, size: (0, 0) }
    }

    /// Makes the targets of `chain`, described by `desc`, on `device`.
    pub(crate) fn create(
        &mut self,
        device: &OverlayDevice,
        chain: &IDXGISwapChain,
        desc: &DXGI_SWAP_CHAIN_DESC,
    ) -> windows::core::Result<()> {
        match &device.on12 {
            Some(on12) => {
                let count = (desc.BufferCount as usize).min(Self::MAX_BUFFERS);
                for index in 0..count {
                    // SAFETY: the chain is live and has `BufferCount` buffers
                    let buffer: ID3D12Resource = unsafe { chain.GetBuffer(index as u32) }?;
                    let flags =
                        D3D11_RESOURCE_FLAGS { BindFlags: D3D11_BIND_RENDER_TARGET.0 as u32, ..Default::default() };
                    let mut wrapped: Option<ID3D11Resource> = None;
                    // SAFETY: the buffer is a live back buffer, kept in the PRESENT state outside a draw; the flags
                    // and out pointer live for the call
                    unsafe {
                        on12.CreateWrappedResource(
                            &buffer,
                            &flags,
                            D3D12_RESOURCE_STATE_PRESENT,
                            D3D12_RESOURCE_STATE_PRESENT,
                            &mut wrapped,
                        )
                    }?;
                    let wrapped = wrapped.ok_or(E_POINTER)?;
                    self.views[index] = Some(Self::view(device, &wrapped)?);
                    self.wrapped[index] = Some(wrapped);
                }
                self.count = count;
            }
            None => {
                // SAFETY: the chain is live and has buffer 0
                let buffer: ID3D11Texture2D = unsafe { chain.GetBuffer(0) }?;
                self.views[0] = Some(Self::view(device, &buffer.cast()?)?);
                self.count = 1;
            }
        }
        self.chain = chain.as_raw() as usize;
        self.size = (desc.BufferDesc.Width, desc.BufferDesc.Height);
        Ok(())
    }

    /// Drops every view and wrapper.
    pub(crate) fn release(&mut self) {
        for index in 0..Self::MAX_BUFFERS {
            self.views[index] = None;
            self.wrapped[index] = None;
        }
        self.count = 0;
        self.chain = 0;
        self.size = (0, 0);
    }

    /// True when these are the targets of `chain`.
    pub(crate) fn is_chain(&self, chain: &IDXGISwapChain) -> bool {
        self.chain != 0 && self.chain == chain.as_raw() as usize
    }

    /// The view of back buffer `index`, and its D3D11On12 wrapper on the D3D12 path.
    pub(crate) fn target(&self, index: usize) -> Option<(&ID3D11RenderTargetView, Option<&ID3D11Resource>)> {
        if index >= self.count {
            return None;
        }
        self.views[index].as_ref().map(|view| (view, self.wrapped[index].as_ref()))
    }

    pub(crate) fn size(&self) -> (u32, u32) {
        self.size
    }

    fn view(device: &OverlayDevice, resource: &ID3D11Resource) -> windows::core::Result<ID3D11RenderTargetView> {
        let mut view: Option<ID3D11RenderTargetView> = None;
        // SAFETY: the resource is a live back buffer made with render target binding
        unsafe { device.device.CreateRenderTargetView(resource, None, Some(&mut view)) }?;
        view.ok_or_else(|| E_POINTER.into())
    }
}
