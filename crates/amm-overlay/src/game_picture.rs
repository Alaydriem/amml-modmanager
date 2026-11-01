use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, ID3D11Device, ID3D11DeviceContext,
    ID3D11Resource, ID3D11ShaderResourceView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{
    DXGI_FORMAT, DXGI_FORMAT_B8G8R8A8_UNORM, DXGI_FORMAT_B8G8R8A8_UNORM_SRGB, DXGI_FORMAT_B8G8R8X8_UNORM,
    DXGI_FORMAT_B8G8R8X8_UNORM_SRGB, DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_FORMAT_R8G8B8A8_UNORM_SRGB, DXGI_FORMAT_UNKNOWN,
    DXGI_SAMPLE_DESC,
};
use windows::core::Interface;

/// The live game picture a layer may draw (`AMM_HOST_GAME_PICTURE_TEXTURE`): one texture of the back buffer's
/// size, filled with one GPU copy at a Present whose host frame asks for it, after the game has drawn and before
/// any layer. Render thread only. The texture is made at the first copy, made again when the back buffer's size
/// or format changes, and dropped on `release`. An sRGB back buffer is viewed through its UNORM format, so the
/// layer shader reads the same gamma-space bytes it writes. A multisampled back buffer gets no picture.
pub struct GamePicture {
    texture: Option<ID3D11Texture2D>,
    view: Option<ID3D11ShaderResourceView>,
    // the back buffer size and format the texture was made for
    width: u32,
    height: u32,
    format: DXGI_FORMAT,
    // set after the first failure to make the texture, so a failing device is logged once per release
    failure_logged: bool,
}

impl GamePicture {
    pub fn new() -> Self {
        Self { texture: None, view: None, width: 0, height: 0, format: DXGI_FORMAT_UNKNOWN, failure_logged: false }
    }

    /// Copies `back_buffer` into the picture texture. On D3D11On12 call it while the wrapped back buffer is
    /// acquired. A back buffer that is not a 2D texture, or is multisampled, is not copied.
    pub fn update(&mut self, device: &ID3D11Device, ctx: &ID3D11DeviceContext, back_buffer: &ID3D11Resource) {
        let Ok(buffer) = back_buffer.cast::<ID3D11Texture2D>() else {
            return;
        };
        let mut source = D3D11_TEXTURE2D_DESC::default();
        // SAFETY: `buffer` is a live texture and `source` is writable for the call
        unsafe { buffer.GetDesc(&mut source) };
        if source.SampleDesc.Count != 1 {
            return;
        }
        if !self.matches(&source) && !self.create(device, &source) {
            return;
        }
        if let Some(texture) = &self.texture {
            // SAFETY: both resources are live, of the same size, and of formats in the same family
            unsafe { ctx.CopyResource(texture, back_buffer) };
        }
    }

    /// The view of the last copy, or `None` when there is none.
    pub fn view(&self) -> Option<&ID3D11ShaderResourceView> {
        self.view.as_ref()
    }

    /// Drops the texture and its view, for a resize, a device loss or shutdown.
    pub fn release(&mut self) {
        self.drop_texture();
        self.failure_logged = false;
    }

    /// The view as the one-element slice a draw binds, or `None` before the first copy.
    pub(crate) fn bound_view(&self) -> Option<&[Option<ID3D11ShaderResourceView>]> {
        self.view.as_ref().map(|_| std::slice::from_ref(&self.view))
    }

    // the format the picture is sampled as: CopyResource accepts a texture of the same format family, and the
    // layer shader writes gamma-space colour, so an sRGB back buffer is read as UNORM
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
    }

    fn matches(&self, source: &D3D11_TEXTURE2D_DESC) -> bool {
        self.texture.is_some()
            && source.Width == self.width
            && source.Height == self.height
            && source.Format == self.format
    }

    fn create(&mut self, device: &ID3D11Device, source: &D3D11_TEXTURE2D_DESC) -> bool {
        self.drop_texture();
        let desc = D3D11_TEXTURE2D_DESC {
            Width: source.Width,
            Height: source.Height,
            MipLevels: 1,
            ArraySize: 1,
            Format: Self::view_format(source.Format),
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let Some((texture, view)) = Self::create_pair(device, &desc) else {
            if !self.failure_logged {
                self.failure_logged = true;
                curia::warn!("game picture texture could not be made, layers that draw it show nothing", {
                    width: source.Width,
                    height: source.Height,
                    format: source.Format.0
                });
            }
            return false;
        };
        self.texture = Some(texture);
        self.view = Some(view);
        self.width = source.Width;
        self.height = source.Height;
        self.format = source.Format;
        true
    }

    fn create_pair(
        device: &ID3D11Device,
        desc: &D3D11_TEXTURE2D_DESC,
    ) -> Option<(ID3D11Texture2D, ID3D11ShaderResourceView)> {
        let mut texture: Option<ID3D11Texture2D> = None;
        // SAFETY: desc lives for the call and the texture has no initial data
        unsafe { device.CreateTexture2D(desc, None, Some(&mut texture)) }.ok()?;
        let texture = texture?;
        let mut view: Option<ID3D11ShaderResourceView> = None;
        // SAFETY: the texture is live and was made with D3D11_BIND_SHADER_RESOURCE
        unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut view)) }.ok()?;
        Some((texture, view?))
    }
}

impl Default for GamePicture {
    fn default() -> Self {
        Self::new()
    }
}
