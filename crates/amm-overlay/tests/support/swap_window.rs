use windows::Win32::Foundation::{HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDeviceAndSwapChain, ID3D11Device, ID3D11DeviceContext,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_MODE_DESC, DXGI_SAMPLE_DESC,
};
use windows::Win32::Graphics::Dxgi::{
    DXGI_PRESENT, DXGI_PRESENT_PARAMETERS, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT,
    DXGI_SWAP_EFFECT_DISCARD, DXGI_SWAP_EFFECT_FLIP_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter,
    IDXGISwapChain, IDXGISwapChain1,
};
use windows::core::Interface;

use super::test_window::TestWindow;

/// A hidden window with a WARP D3D11 device and a swap chain of `SIZE` x `SIZE` R8G8B8A8 pixels. The chain drops
/// before the window.
pub struct SwapWindow {
    pub chain: IDXGISwapChain,
    pub ctx: ID3D11DeviceContext,
    pub device: ID3D11Device,
    pub window: TestWindow,
}

impl SwapWindow {
    pub const SIZE: u32 = 64;

    /// A flip chain of two buffers, as the game makes.
    pub fn new() -> Self {
        Self::with_effect(DXGI_SWAP_EFFECT_FLIP_DISCARD, 2)
    }

    /// A bitblt chain of one buffer. A window can hold more than one of them, which a flip chain does not allow
    /// while the overlay still holds a view of the old chain's back buffer.
    pub fn bitblt() -> Self {
        Self::with_effect(DXGI_SWAP_EFFECT_DISCARD, 1)
    }

    /// A new device and bitblt chain on the window of `self`.
    pub fn rebuilt(self) -> Self {
        let SwapWindow { chain, ctx, device, window } = self;
        drop(chain);
        drop(ctx);
        drop(device);
        let (chain, device, ctx) = Self::chain_on(window.hwnd, DXGI_SWAP_EFFECT_DISCARD, 1);
        Self { chain, ctx, device, window }
    }

    fn with_effect(effect: DXGI_SWAP_EFFECT, buffers: u32) -> Self {
        let window = TestWindow::new(Self::SIZE);
        let (chain, device, ctx) = Self::chain_on(window.hwnd, effect, buffers);
        Self { chain, ctx, device, window }
    }

    fn chain_on(
        hwnd: HWND,
        effect: DXGI_SWAP_EFFECT,
        buffers: u32,
    ) -> (IDXGISwapChain, ID3D11Device, ID3D11DeviceContext) {
        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC {
                Width: Self::SIZE,
                Height: Self::SIZE,
                Format: DXGI_FORMAT_R8G8B8A8_UNORM,
                ..Default::default()
            },
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: buffers,
            OutputWindow: hwnd,
            Windowed: true.into(),
            SwapEffect: effect,
            Flags: 0,
        };
        let mut chain = None;
        let mut device = None;
        let mut ctx = None;
        // SAFETY: desc and the out pointers live for the call
        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None::<&IDXGIAdapter>,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&desc),
                Some(&mut chain),
                Some(&mut device),
                None,
                Some(&mut ctx),
            )
        }
        .unwrap();
        (chain.unwrap(), device.unwrap(), ctx.unwrap())
    }

    /// Presents; the result is not checked, since a hidden window may report itself occluded.
    pub fn present(&self) {
        self.present_with(DXGI_PRESENT(0));
    }

    pub fn present_with(&self, flags: DXGI_PRESENT) {
        // SAFETY: the chain is live
        let _ = unsafe { self.chain.Present(0, flags) };
    }

    pub fn present1(&self) {
        let chain: IDXGISwapChain1 = self.chain.cast().unwrap();
        let params = DXGI_PRESENT_PARAMETERS::default();
        // SAFETY: the chain and params are live
        let _ = unsafe { chain.Present1(0, DXGI_PRESENT(0), &params) };
    }

    pub fn resize(&self, width: u32, height: u32) -> windows::core::Result<()> {
        // SAFETY: the chain is live; 0 buffers keeps the count
        unsafe { self.chain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0)) }
    }

    /// How many times Present or Present1 of this chain reached DXGI.
    pub fn present_count(&self) -> u32 {
        // SAFETY: the chain is live
        unsafe { self.chain.GetLastPresentCount() }.unwrap()
    }
}
