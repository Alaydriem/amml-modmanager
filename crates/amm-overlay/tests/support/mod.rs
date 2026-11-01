#![allow(clippy::unwrap_used)]

pub mod cross_thread;
pub mod d3d12_window;
pub mod log_capture;
pub mod message_log;
pub mod quiet_panics;
pub mod ref_count;
pub mod retour_installer;
pub mod swap_window;
pub mod test_host;
pub mod test_sink;
pub mod test_window;

use amm_abi::types::{AmmDrawCmd, AmmMeshView, AmmSamplerKind, AmmTextureDelta, AmmVertex};
use amm_ffi::types::{AmmHostFrame, AmmHostLayer};
use windows::Win32::Foundation::HMODULE;
use windows::Win32::Graphics::Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_11_0};
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE, D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_FLAG,
    D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    D3D11_USAGE_STAGING, D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RenderTargetView, ID3D11Resource,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};
use windows::core::Interface;

/// A WARP device and an offscreen R8G8B8A8 target of `SIZE` x `SIZE` pixels.
pub struct WarpTarget {
    pub device: ID3D11Device,
    pub ctx: ID3D11DeviceContext,
    pub texture: ID3D11Texture2D,
    pub view: ID3D11RenderTargetView,
}

impl WarpTarget {
    pub const SIZE: u32 = 64;

    pub fn new() -> Self {
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
        let ctx = ctx.unwrap();
        let texture = Self::texture(&device, Self::SIZE, Self::SIZE, DXGI_FORMAT_R8G8B8A8_UNORM);
        let view = Self::render_view(&device, &texture);
        Self { device, ctx, texture, view }
    }

    pub fn size(&self) -> (u32, u32) {
        (Self::SIZE, Self::SIZE)
    }

    pub fn resource(&self) -> ID3D11Resource {
        self.texture.cast().unwrap()
    }

    /// A render target texture that can also be sampled.
    pub fn texture(device: &ID3D11Device, width: u32, height: u32, format: DXGI_FORMAT) -> ID3D11Texture2D {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: format,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let mut texture = None;
        // SAFETY: desc lives for the call
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut texture)) }.unwrap();
        texture.unwrap()
    }

    pub fn render_view(device: &ID3D11Device, texture: &ID3D11Texture2D) -> ID3D11RenderTargetView {
        let mut view = None;
        // SAFETY: the texture was made with D3D11_BIND_RENDER_TARGET
        unsafe { device.CreateRenderTargetView(texture, None, Some(&mut view)) }.unwrap();
        view.unwrap()
    }

    pub fn clear(&self, rgba: [u8; 4]) {
        Self::clear_view(&self.ctx, &self.view, rgba);
    }

    pub fn clear_view(ctx: &ID3D11DeviceContext, view: &ID3D11RenderTargetView, rgba: [u8; 4]) {
        let color = rgba.map(|channel| f32::from(channel) / 255.0);
        // SAFETY: the view is live
        unsafe { ctx.ClearRenderTargetView(view, &color) };
    }

    /// Every pixel of the target, row by row.
    pub fn pixels(&self) -> Vec<[u8; 4]> {
        Self::read(&self.device, &self.ctx, &self.texture, Self::SIZE, Self::SIZE)
    }

    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        self.pixels()[(y * Self::SIZE + x) as usize]
    }

    pub fn read(
        device: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        texture: &ID3D11Texture2D,
        width: u32,
        height: u32,
    ) -> Vec<[u8; 4]> {
        let mut source = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: the texture is live
        unsafe { texture.GetDesc(&mut source) };
        let desc = D3D11_TEXTURE2D_DESC {
            BindFlags: 0,
            Usage: D3D11_USAGE_STAGING,
            CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
            ..source
        };
        let mut staging: Option<ID3D11Texture2D> = None;
        // SAFETY: desc lives for the call
        unsafe { device.CreateTexture2D(&desc, None, Some(&mut staging)) }.unwrap();
        let staging = staging.unwrap();
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        // SAFETY: both textures are live and match; the map blocks until the copy is done
        unsafe {
            ctx.CopyResource(&staging, texture);
            ctx.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped)).unwrap();
        }
        let mut pixels = Vec::with_capacity((width * height) as usize);
        for y in 0..height as usize {
            // SAFETY: the mapped texture holds `height` rows of RowPitch bytes
            let row = unsafe {
                std::slice::from_raw_parts(
                    mapped.pData.cast::<u8>().add(y * mapped.RowPitch as usize),
                    width as usize * 4,
                )
            };
            pixels.extend(row.as_chunks::<4>().0.iter().copied());
        }
        // SAFETY: mapped above
        unsafe { ctx.Unmap(&staging, 0) };
        pixels
    }
}

/// The owned arrays of one layer's mesh, so the `AmmMeshView` of a test stays valid while it is drawn.
#[derive(Default)]
pub struct TestMesh {
    pub vertices: Vec<AmmVertex>,
    pub indices: Vec<u32>,
    pub cmds: Vec<AmmDrawCmd>,
    pub deltas: Vec<AmmTextureDelta>,
    pub bytes: Vec<u8>,
    pub freed: Vec<u64>,
}

impl TestMesh {
    pub const FULL_CLIP: [f32; 4] = [0.0, 0.0, 64.0, 64.0];

    /// Adds a quad over `rect` (`[min_x, min_y, max_x, max_y]` in pixels) with UVs 0 to 1, and a command for it.
    pub fn quad(&mut self, rect: [f32; 4], color: [u8; 4], texture: u64, clip: [f32; 4]) -> &mut Self {
        self.quad_with(rect, color, texture, clip, 0, AmmSamplerKind::Linear as u8)
    }

    pub fn quad_with(
        &mut self,
        rect: [f32; 4],
        color: [u8; 4],
        texture: u64,
        clip: [f32; 4],
        shader: u8,
        sampler: u8,
    ) -> &mut Self {
        let base = self.vertices.len() as u32;
        let offset = self.indices.len() as u32;
        let [x0, y0, x1, y1] = rect;
        for (pos, uv) in
            [([x0, y0], [0.0, 0.0]), ([x1, y0], [1.0, 0.0]), ([x1, y1], [1.0, 1.0]), ([x0, y1], [0.0, 1.0])]
        {
            self.vertices.push(AmmVertex { pos, uv, color });
        }
        self.indices.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        self.cmds.push(AmmDrawCmd { texture, index_offset: offset, index_count: 6, clip, shader, sampler });
        self
    }

    /// Adds a full delta that makes texture `id` of `width` x `height` pixels, all `rgba`.
    pub fn full(&mut self, id: u64, width: u32, height: u32, rgba: [u8; 4]) -> &mut Self {
        self.delta(id, 1, 0, 0, width, height, rgba)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn delta(&mut self, id: u64, full: u8, x: u32, y: u32, width: u32, height: u32, rgba: [u8; 4]) -> &mut Self {
        let byte_offset = self.bytes.len() as u32;
        for _ in 0..width * height {
            self.bytes.extend(rgba);
        }
        self.deltas.push(AmmTextureDelta {
            id,
            sampler: AmmSamplerKind::Linear as u8,
            full,
            x,
            y,
            width,
            height,
            byte_offset,
            byte_len: width * height * 4,
        });
        self
    }

    pub fn view(&self) -> AmmMeshView {
        AmmMeshView {
            vertices: self.vertices.as_ptr(),
            vertex_count: self.vertices.len(),
            indices: self.indices.as_ptr(),
            index_count: self.indices.len(),
            cmds: self.cmds.as_ptr(),
            cmd_count: self.cmds.len(),
            deltas: self.deltas.as_ptr(),
            delta_count: self.deltas.len(),
            bytes: self.bytes.as_ptr(),
            byte_count: self.bytes.len(),
            freed: self.freed.as_ptr(),
            freed_count: self.freed.len(),
        }
    }
}

/// The layers of one host frame and the frame that points at them.
pub struct TestFrame {
    pub layers: Vec<AmmHostLayer>,
}

impl TestFrame {
    pub fn new(layers: &[(u8, &TestMesh)]) -> Self {
        Self { layers: layers.iter().map(|(owner, mesh)| AmmHostLayer { owner: *owner, mesh: mesh.view() }).collect() }
    }

    pub fn frame(&self) -> AmmHostFrame {
        AmmHostFrame { layers: self.layers.as_ptr(), layer_count: self.layers.len() as u32, ..AmmHostFrame::empty() }
    }
}

/// Checks two pixels channel by channel within `tolerance`.
pub struct PixelCheck;

impl PixelCheck {
    pub fn near(actual: [u8; 4], expected: [u8; 4], tolerance: u8) -> bool {
        actual.iter().zip(expected.iter()).all(|(a, e)| a.abs_diff(*e) <= tolerance)
    }
}
