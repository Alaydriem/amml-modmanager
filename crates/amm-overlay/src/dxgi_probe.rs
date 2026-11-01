use windows::Win32::Foundation::{E_POINTER, HMODULE, HWND};
use windows::Win32::Graphics::Direct3D::{
    D3D_DRIVER_TYPE, D3D_DRIVER_TYPE_HARDWARE, D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0,
};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_CREATE_DEVICE_FLAG, D3D11_SDK_VERSION, D3D11CreateDeviceAndSwapChain,
};
use windows::Win32::Graphics::Direct3D12::{
    D3D12_COMMAND_LIST_TYPE_DIRECT, D3D12_COMMAND_QUEUE_DESC, D3D12CreateDevice, ID3D12CommandQueue, ID3D12Device,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_MODE_DESC, DXGI_SAMPLE_DESC};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_SWAP_CHAIN_DESC, DXGI_SWAP_EFFECT_DISCARD, DXGI_USAGE_RENDER_TARGET_OUTPUT, IDXGIAdapter,
    IDXGIFactory4, IDXGISwapChain, IDXGISwapChain3,
};
use windows::core::{IUnknown, Interface};

use crate::overlay_error::OverlayError;
use crate::probe_targets::ProbeTargets;
use crate::probe_window::ProbeWindow;

/// Reads the addresses to hook from the vtables of throwaway objects: a D3D11 swap chain on a hidden 8x8 window of
/// our own, and a D3D12 DIRECT queue. The indices are fixed by the DXGI and D3D12 ABI. The hardware driver is tried
/// first, then WARP; the vtables are the same for both.
pub struct DxgiProbe;

impl DxgiProbe {
    pub const PRESENT_INDEX: usize = 8;
    pub const RESIZE_BUFFERS_INDEX: usize = 13;
    pub const PRESENT1_INDEX: usize = 22;
    pub const RESIZE_BUFFERS1_INDEX: usize = 39;
    pub const EXECUTE_COMMAND_LISTS_INDEX: usize = 10;

    /// The swap chain entries, and the queue entry when a probe queue could be made.
    pub fn targets() -> Result<ProbeTargets, OverlayError> {
        let window = ProbeWindow::new()?;
        let chain = Self::probe_chain(window.hwnd())?;
        // SAFETY: `chain` is a live IDXGISwapChain3, whose vtable holds every IDXGISwapChain3 entry up to index 39
        let (present, resize_buffers, present1, resize_buffers1) = unsafe {
            (
                Self::entry(&chain, Self::PRESENT_INDEX),
                Self::entry(&chain, Self::RESIZE_BUFFERS_INDEX),
                Self::entry(&chain, Self::PRESENT1_INDEX),
                Self::entry(&chain, Self::RESIZE_BUFFERS1_INDEX),
            )
        };
        // the chain goes before its window
        drop(chain);
        drop(window);
        Ok(ProbeTargets {
            present,
            resize_buffers,
            present1,
            resize_buffers1,
            execute_command_lists: Self::queue_entry(),
        })
    }

    fn probe_chain(window: HWND) -> Result<IDXGISwapChain3, OverlayError> {
        Self::chain_on(window, D3D_DRIVER_TYPE_HARDWARE)
            .or_else(|_| Self::chain_on(window, D3D_DRIVER_TYPE_WARP))
            .map_err(|source| OverlayError::Probe { step: "probe swap chain", source })
    }

    fn chain_on(window: HWND, driver: D3D_DRIVER_TYPE) -> windows::core::Result<IDXGISwapChain3> {
        let desc = DXGI_SWAP_CHAIN_DESC {
            BufferDesc: DXGI_MODE_DESC { Format: DXGI_FORMAT_R8G8B8A8_UNORM, ..Default::default() },
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            BufferUsage: DXGI_USAGE_RENDER_TARGET_OUTPUT,
            BufferCount: 1,
            OutputWindow: window,
            Windowed: true.into(),
            SwapEffect: DXGI_SWAP_EFFECT_DISCARD,
            Flags: 0,
        };
        let mut chain: Option<IDXGISwapChain> = None;
        // SAFETY: `desc` and the feature level slice live for the call, and the out pointer is a writable local
        unsafe {
            D3D11CreateDeviceAndSwapChain(
                None::<&IDXGIAdapter>,
                driver,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&desc),
                Some(&mut chain),
                None,
                None,
                None,
            )
        }?;
        chain.ok_or_else(|| windows::core::Error::from(E_POINTER))?.cast()
    }

    // optional: a failure is logged and leaves the queue entry unhooked
    fn queue_entry() -> Option<usize> {
        let Some(queue) = Self::probe_queue() else {
            curia::warn!("overlay could not make the probe D3D12 queue, it can draw only on a D3D11 game");
            return None;
        };
        // SAFETY: `queue` is a live ID3D12CommandQueue, whose vtable holds ExecuteCommandLists at index 10
        Some(unsafe { Self::entry(&queue, Self::EXECUTE_COMMAND_LISTS_INDEX) })
    }

    fn probe_queue() -> Option<ID3D12CommandQueue> {
        let device = Self::d3d12_device(None)
            .or_else(|| Self::warp_adapter().and_then(|adapter| Self::d3d12_device(Some(&adapter))))?;
        let desc = D3D12_COMMAND_QUEUE_DESC { Type: D3D12_COMMAND_LIST_TYPE_DIRECT, ..Default::default() };
        // SAFETY: `desc` lives for the call
        unsafe { device.CreateCommandQueue(&desc) }.ok()
    }

    fn d3d12_device(adapter: Option<&IUnknown>) -> Option<ID3D12Device> {
        let mut device: Option<ID3D12Device> = None;
        // SAFETY: the out pointer is a writable local and the adapter, when given, is live
        unsafe { D3D12CreateDevice(adapter, D3D_FEATURE_LEVEL_11_0, &mut device) }.ok()?;
        device
    }

    fn warp_adapter() -> Option<IUnknown> {
        // SAFETY: a factory takes no input
        let factory: IDXGIFactory4 = unsafe { CreateDXGIFactory1() }.ok()?;
        // SAFETY: the factory is live
        unsafe { factory.EnumWarpAdapter() }.ok()
    }

    /// The function at `index` of the vtable of `object`.
    ///
    /// # Safety
    ///
    /// `object` is live and its vtable has more than `index` entries.
    unsafe fn entry(object: &impl Interface, index: usize) -> usize {
        // SAFETY: a COM object starts with its vtable pointer, and the caller vouches for the length
        unsafe { *(*(object.as_raw() as *const *const usize)).add(index) }
    }
}
