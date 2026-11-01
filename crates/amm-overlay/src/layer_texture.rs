use windows::Win32::Graphics::Direct3D11::{ID3D11ShaderResourceView, ID3D11Texture2D};

/// One registry texture. `view` is always `Some`; it is kept as an `Option` so a draw binds it as a one-element
/// slice without taking a reference.
pub(crate) struct LayerTexture {
    pub texture: ID3D11Texture2D,
    pub view: Option<ID3D11ShaderResourceView>,
    pub width: u32,
    pub height: u32,
}
