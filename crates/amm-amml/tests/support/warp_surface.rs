use amml_api::amml_abi::AmmlDrawSurface;
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG,
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::core::Interface;

/// A WARP device and an offscreen R8G8B8A8 target of `width` x `height` pixels, lent to the manager as the surface
/// of a `FramePresent`, as the loader lends the game's back buffer.
pub struct WarpSurface {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub texture: ID3D11Texture2D,
    pub view: ID3D11RenderTargetView,
    pub width: u32,
    pub height: u32,
}

impl WarpSurface {
    pub fn new(width: u32, height: u32) -> Self {
        let mut device = None;
        let mut ctx = None;
        // SAFETY: the out pointers are writable locals and the feature level slice lives for the call
        unsafe {
            D3D11CreateDevice(
                None,
                D3D_DRIVER_TYPE_WARP,
                HMODULE::default(),
                D3D11_CREATE_DEVICE_FLAG(0),
                Some(&[D3D_FEATURE_LEVEL_11_0]),
                D3D11_SDK_VERSION,
                Some(&mut device),
                None,
                Some(&mut ctx),
            )
        }
        .unwrap();
        let device: ID3D11Device = device.unwrap();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: desc lives for the call
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.unwrap();
        let texture: ID3D11Texture2D = texture.unwrap();
        let mut view = None;
        // SAFETY: the texture was made with D3D11_BIND_RENDER_TARGET
        unsafe { device.CreateRenderTargetView(&texture, None, Some(&mut view)) }.unwrap();
        Self { device, ctx: ctx.unwrap(), texture, view: view.unwrap(), width, height }
    }

    /// The draw surface of this target for device epoch `epoch`, as the loader writes it into a `FramePresent`.
    pub fn surface(&self, epoch: u64) -> AmmlDrawSurface {
        AmmlDrawSurface {
            size: AmmlDrawSurface::SIZE_V1,
            version: 1,
            width: self.width,
            height: self.height,
            format: DXGI_FORMAT_R8G8B8A8_UNORM.0 as u32,
            wrapped: 0,
            _reserved: [0; 3],
            device_epoch: epoch,
            device: self.device.as_raw(),
            context: self.ctx.as_raw(),
            target: self.view.as_raw(),
            back_buffer: self.texture.as_raw(),
        }
    }

    /// Clears the target to opaque black.
    pub fn clear(&self) {
        // SAFETY: the view is live
        unsafe { self.ctx.ClearRenderTargetView(&self.view, &[0.0, 0.0, 0.0, 1.0]) };
    }

    /// How many pixels of the target are not opaque black.
    pub fn drawn_pixels(&self) -> usize {
        let mut source = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: the texture is live
        unsafe { self.texture.GetDesc(&mut source) };
        let desc = D3D11_TEXTURE2D_DESC {
            BindFlags: 0,
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..source
        };
        let mut staging: Option<ID3D11Texture2D> = None;
        // SAFETY: desc lives for the call
        unsafe { self.device.CreateTexture2D(&desc, None, Some(&mut staging)) }.unwrap();
        let staging = staging.unwrap();
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: both textures are live and match; the map blocks until the copy is done
        unsafe {
            self.ctx.CopyResource(&staging, &self.texture);
            self.ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).unwrap();
        }
        let mut drawn = 0;
        for y in 0..self.height as usize {
            // SAFETY: the mapped texture holds `height` rows of RowPitch bytes
            let row = unsafe {
                std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                    self.width as usize * 4,
                )
            };
            drawn += row.as_chunks::<4>().0.iter().filter(|pixel| **pixel != [0, 0, 0, 255]).count();
        }
        // SAFETY: mapped above
        unsafe { self.ctx.Unmap(&staging, 0) };
        drawn
    }
}
