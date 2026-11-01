use windows::Win32::Graphics::Direct3D11::{D3D11_CREATE_DEVICE_BGRA_SUPPORT, ID3D11Device, ID3D11DeviceContext};
use windows::Win32::Graphics::Direct3D11on12::{D3D11On12CreateDevice, ID3D11On12Device};
use windows::Win32::Graphics::Direct3D12::ID3D12Device;
use windows::Win32::Graphics::Dxgi::IDXGISwapChain;
use windows::core::{IUnknown, Interface};

use crate::device_start::DeviceStart;
use crate::queue_slot::QueueSlot;

/// The D3D11 device the overlay draws with. On a D3D11 chain it is the game's own device and immediate context; on
/// a D3D12 chain it is a D3D11On12 device made on the chain's device, drawing through the game's DIRECT queue.
pub(crate) struct OverlayDevice {
    pub(crate) device: ID3D11Device,
    pub(crate) ctx: ID3D11DeviceContext,
    /// Set on the D3D12 path.
    pub(crate) on12: Option<ID3D11On12Device>,
}

impl OverlayDevice {
    /// The device for `chain`: its own D3D11 device, or a D3D11On12 device on the queue `queue` holds, which must
    /// belong to the chain's device.
    pub(crate) fn start(chain: &IDXGISwapChain, queue: &QueueSlot) -> DeviceStart {
        // SAFETY: the chain is live for the Present that called this
        if let Ok(device) = unsafe { chain.GetDevice::<ID3D11Device>() } {
            // SAFETY: the device is live
            return match unsafe { device.GetImmediateContext() } {
                Ok(ctx) => DeviceStart::Ready(Self { device, ctx, on12: None }),
                Err(_) => {
                    curia::error!("overlay could not get the game's D3D11 context");
                    DeviceStart::Failed
                }
            };
        }
        Self::start_on12(chain, queue)
    }

    pub(crate) fn is_on12(&self) -> bool {
        self.on12.is_some()
    }

    fn start_on12(chain: &IDXGISwapChain, slot: &QueueSlot) -> DeviceStart {
        let Some(queue) = slot.get() else {
            return DeviceStart::Waiting;
        };
        // SAFETY: the chain is live for the Present that called this
        let Ok(chain_device) = (unsafe { chain.GetDevice::<ID3D12Device>() }) else {
            curia::error!("overlay swap chain has neither a D3D11 nor a D3D12 device");
            return DeviceStart::Failed;
        };
        let mut queue_device: Option<ID3D12Device> = None;
        // SAFETY: the queue is live and the out pointer is a writable local
        let queue_device = unsafe { queue.GetDevice(&mut queue_device) }.ok().and(queue_device);
        if queue_device.as_ref() != Some(&chain_device) {
            slot.forget(&queue);
            return DeviceStart::Waiting;
        }
        let Ok(queue) = queue.cast::<IUnknown>() else {
            return DeviceStart::Failed;
        };
        let queues = [Some(queue)];
        let mut device: Option<ID3D11Device> = None;
        let mut ctx: Option<ID3D11DeviceContext> = None;
        // SAFETY: the device and queue are live, and the out pointers are writable locals
        let made = unsafe {
            D3D11On12CreateDevice(
                &chain_device,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT.0,
                None,
                Some(&queues),
                0,
                Some(&mut device),
                Some(&mut ctx),
                None,
            )
        };
        let on12 = device.as_ref().and_then(|device| device.cast::<ID3D11On12Device>().ok());
        match (made, device, ctx, on12) {
            (Ok(()), Some(device), Some(ctx), Some(on12)) => DeviceStart::Ready(Self { device, ctx, on12: Some(on12) }),
            _ => {
                curia::error!("overlay could not create a D3D11On12 device");
                DeviceStart::Failed
            }
        }
    }
}
