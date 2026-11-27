use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11ShaderResourceView,
    ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_FORMAT_B8G8R8X8_UNORM,
    DXGI_FORMAT_B8G8R8X8_UNORM_SRGB, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB,
    DXGI_FORMAT_R10G10B10A2_UNORM, DXGI_FORMAT_R16G16B16A16_FLOAT, DXGI_FORMAT_UNKNOWN, DXGI_SAMPLE_DESC,
};

use crate::overlay_error::OverlayError;

/// The live game picture a layer may draw (`AMM_HOST_GAME_PICTURE_TEXTURE`): one texture of the back buffer's
/// size, which the loader fills with one copy of the back buffer (`DrawSurface::copy_back_buffer`) at a frame whose
/// host frame asks for it, before any layer. Render thread only. The texture is made by `prepare`, made again when
/// the back buffer's size or format changes, and dropped on `release`. An sRGB back buffer is viewed through its
/// UNORM twin, so the layer shader reads the same gamma-space bytes it writes. Only a texture whose last copy
/// succeeded is drawn.
pub struct GamePicture {
    texture: Option<ID3D11Texture2D>,
    view: Option<ID3D11ShaderResourceView>,
    // the back buffer size and format the texture was made for
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    // the last copy into the texture succeeded
    copied: bool,
    // set after the first failure to make the texture, so a failing device is logged once per release
    failure_logged: bool,
}

impl GamePicture {
    pub fn new() -> Self {
        Self {
            texture: None,
            view: None,
            width: 0,
            height: 0,
            format: DXGI_FORMAT_UNKNOWN,
            copied: false,
            failure_logged: false,
        }
    }

    /// The texture to copy a back buffer of `width` x `height` in `format` into, made on `device` when there is
    /// none for that size and format. A remade texture holds no copy until `set_copied(true)`. A size of 0 or a
    /// format the picture cannot sample makes nothing.
    pub fn prepare(
        &mut self,
        device: &ID3D11Device,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<&ID3D11Texture2D, OverlayError> {
        if width == 0 || height == 0 || !Self::supported(format) {
            return Err(OverlayError::PictureSurface { width, height, format: format.0 as u32 });
        }
        if !self.matches(width, height, format) {
            self.create(device, width, height, format)?;
        }
        self.texture.as_ref().ok_or(OverlayError::NoDevice)
    }

    /// Whether the copy into the texture `prepare` gave succeeded. A failed copy is not drawn.
    pub fn set_copied(&mut self, copied: bool) {
        self.copied = copied && self.texture.is_some();
    }

    /// The view of the last successful copy, or `None` when there is none.
    pub fn view(&self) -> Option<&ID3D11ShaderResourceView> {
        self.view.as_ref().filter(|_| self.copied)
    }

    /// The device objects held: the texture and its view, or none.
    pub fn object_count(&self) -> usize {
        usize::from(self.texture.is_some()) + usize::from(self.view.is_some())
    }

    /// Drops the texture and its view, for a device loss or shutdown.
    pub fn release(&mut self) {
        self.drop_texture();
        self.failure_logged = false;
    }

    /// The view as the one-element slice a draw binds, or `None` before the first successful copy.
    pub(crate) fn bound_view(&self) -> Option<&[Option<ID3D11ShaderResourceView>]> {
        self.view().map(|_| std::slice::from_ref(&self.view))
    }

    // the formats a swap chain back buffer of the game can have that a shader can sample
    fn supported(format: DXGI_FORMAT) -> bool {
        matches!(
            format,
            DXGI_FORMAT_R8G8B8A8_UNORM
                | DXGI_FORMAT_R8G8B8A8_UNORM_SRGB
                | DXGI_FORMAT_B8G8R8A8_UNORM
                | DXGI_FORMAT_B8G8R8A8_UNORM_SRGB
                | DXGI_FORMAT_B8G8R8X8_UNORM
                | DXGI_FORMAT_B8G8R8X8_UNORM_SRGB
                | DXGI_FORMAT_R10G10B10A2_UNORM
                | DXGI_FORMAT_R16G16B16A16_FLOAT
        )
    }

    // the format the picture is made in: the loader copies into the back buffer's format or its sRGB / UNORM twin,
    // and the layer shader writes gamma-space colour, so an sRGB back buffer is read as UNORM
    fn view_format(format: DXGI_FORMAT) -> DXGI_FORMAT {
        match format {
            DXGI_FORMAT_R8G8B8A8_UNORM_SRGB => DXGI_FORMAT_R8G8B8A8_UNORM,
            DXGI_FORMAT_B8G8R8A8_UNORM_SRGB => DXGI_FORMAT_B8G8R8A8_UNORM,
            DXGI_FORMAT_B8G8R8X8_UNORM_SRGB => DXGI_FORMAT_B8G8R8X8_UNORM,
            other => other,
        }
    }

    fn drop_texture(&mut self) {
        self.view = None;
        self.texture = None;
        self.width = 0;
        self.height = 0;
        self.format = DXGI_FORMAT_UNKNOWN;
        self.copied = false;
    }

    fn matches(&self, width: u32, height: u32, format: DXGI_FORMAT) -> bool {
        self.texture.is_some() && width == self.width && height == self.height && format == self.format
    }

    fn create(
        &mut self,
        device: &ID3D11Device,
        width: u32,
        height: u32,
        format: DXGI_FORMAT,
    ) -> Result<(), OverlayError> {
        self.drop_texture();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: width,
            Height: height,
            MipLevels: 1,
            ArraySize: 1,
            Format: Self::view_format(format),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let (texture, view) = match Self::create_pair(device, &desc) {
            Ok(pair) => pair,
            Err(error) => {
                if !self.failure_logged {
                    self.failure_logged = true;
                    curia::warn!("game picture texture could not be made, layers that draw it show nothing", {
                        width: width,
                        height: height,
                        format: format.0
                    });
                }
                return Err(error);
            }
        };
        self.texture = Some(texture);
        self.view = Some(view);
        self.width = width;
        self.height = height;
        self.format = format;
        Ok(())
    }

    fn create_pair(
        device: &ID3D11Device,
        desc: &D3D11_TEXTURE2D_DESC,
    ) -> Result<(ID3D11Texture2D, ID3D11ShaderResourceView), OverlayError> {
        let mut texture: Option<ID3D11Texture2D> = None;
        // SAFETY: desc lives for the call and the texture has no initial data
        let result = unsafe { device.CreateTexture2D(desc, None, Some(&mut texture)) };
        let texture = OverlayError::created("game picture texture", result, texture)?;
        let mut view: Option<ID3D11ShaderResourceView> = None;
        // SAFETY: the texture is live and was made with D3D11_BIND_SHADER_RESOURCE
        let result = unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut view)) };
        let view = OverlayError::created("game picture view", result, view)?;
        Ok((texture, view))
    }
}

impl Default for GamePicture {
    fn default() -> Self {
        Self::new()
    }
}
