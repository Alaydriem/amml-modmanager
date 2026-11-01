use crate::device::OverlayDevice;

/// What `OverlayDevice::start` made of a swap chain.
pub(crate) enum DeviceStart {
    Ready(OverlayDevice),
    /// A D3D12 chain whose queue was not seen yet, or a queue of another device was dropped; tried again at the
    /// next Present.
    Waiting,
    /// The device cannot be made on this chain; logged, and tried again only after a new device.
    Failed,
}
