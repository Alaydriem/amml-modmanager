use windows::Win32::Foundation::HANDLE;
use windows::Win32::Graphics::Direct3D::D3D_FEATURE_LEVEL_11_0;
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12_FENCE_FLAG_NONE, D3D12CreateDevice,
    ID3D12CommandAllocator, ID3D12CommandList, ID3D12CommandQueue, ID3D12Device, ID3D12Fence,
    ID3D12GraphicsCommandList, ID3D12PipelineState,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_PRESENT, DXGI_SWAP_CHAIN_DESC1, DXGI_SWAP_CHAIN_FLAG, DXGI_SWAP_EFFECT_FLIP_DISCARD,
    DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter, IDXGIFactory4, IDXGIOutput, IDXGISwapChain1,
};
use windows::core::{IUnknown, Interface};

use super::swap_window::SwapWindow;
use super::test_window::TestWindow;

/// A hidden window with a WARP D3D12 device, a DIRECT queue and a flip swap chain of two `SwapWindow::SIZE`
/// square buffers. `submit` runs an empty command list on the queue, as the game does before its first Present.
pub struct D3d12Window {
    pub chain: IDXGISwapChain1,
    list: ID3D12GraphicsCommandList,
    _allocator: ID3D12CommandAllocator,
    fence: ID3D12Fence,
    pub queue: ID3D12CommandQueue,
    _device: ID3D12Device,
    _window: TestWindow,
}

impl D3d12Window {
    pub fn new() -> Self {
        let window = TestWindow::new(SwapWindow::SIZE);
        // SAFETY: a factory takes no input
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1() }.unwrap();
        // SAFETY: the factory is live
        let adapter: IDXGIAdapter = unsafe { factory.EnumWarpAdapter() }.unwrap();
        let mut device: Option<ID3D12Device> = None;
        // SAFETY: the adapter is live and the out pointer is a local
        unsafe { D3D12CreateDevice(&adapter.cast::<IUnknown>().unwrap(), D3D_FEATURE_LEVEL_11_0, &mut device) }
            .unwrap();
        let device = device.unwrap();
        let desc = D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() };
        // SAFETY: desc lives for the call
        let queue: ID3D12CommandQueue = unsafe { device.CreateCommandQueue(&desc) }.unwrap();
        // SAFETY: the device is live
        let allocator: ID3D12CommandAllocator =
            unsafe { device.CreateCommandAllocator(D3D12_COMMAND_LIST_TYPE_DIRECT) }.unwrap();
        // SAFETY: the allocator is live and the list starts with no pipeline state
        let list: ID3D12GraphicsCommandList = unsafe {
            device.CreateCommandList(0, D3D12_COMMAND_LIST_TYPE_DIRECT, &allocator, None::<&ID3D12PipelineState>)
        }
        .unwrap();
        // SAFETY: the list is open
        unsafe { list.Close() }.unwrap();
        // SAFETY: the device is live
        let fence: ID3D12Fence = unsafe { device.CreateFence(0, D3D12_FENCE_FLAG_NONE) }.unwrap();
        let chain_desc = DXGI_SWAP_CHAIN_DESC1 {
            Width: SwapWindow::SIZE,
            Height: SwapWindow::SIZE,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 2,
            SwapEffect: DXGI_SWAP_EFFECT_FLIP_DISCARD,
            ..Default::default()
        };
        // SAFETY: the queue and window are live and the desc lives for the call
        let chain =
            unsafe { factory.CreateSwapChainForHwnd(&queue, window.hwnd, &chain_desc, None, None::<&IDXGIOutput>) }
                .unwrap();
        Self { chain, list, _allocator: allocator, fence, queue, _device: device, _window: window }
    }

    pub fn submit(&self) {
        let lists = [Some(self.list.cast::<ID3D12CommandList>().unwrap())];
        // SAFETY: the list is closed and live
        unsafe { self.queue.ExecuteCommandLists(&lists) };
    }

    pub fn present(&self) {
        // SAFETY: the chain is live
        let _ = unsafe { self.chain.Present(0, DXGI_PRESENT(0)) };
    }

    /// Waits for the queue to finish, then resizes, as a D3D12 game must.
    pub fn resize(&self, width: u32, height: u32) -> windows::core::Result<()> {
        // SAFETY: the fence and queue are live; a null event makes the wait block
        unsafe {
            let value = self.fence.GetCompletedValue() + 1;
            self.queue.Signal(&self.fence, value)?;
            self.fence.SetEventOnCompletion(value, HANDLE::default())?;
            self.chain.ResizeBuffers(0, width, height, DXGI_FORMAT_UNKNOWN, DXGI_SWAP_CHAIN_FLAG(0))
        }
    }
}
