use std::collections::HashMap;

use amm_abi::types::AmmTextureDelta;
use amm_ffi::types::AmmHostLayer;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_BIND_SHADER_RESOURCE, D3D11_BOX, D3D11_SUBRESOURCE_DATA, D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT,
    ID3D11Device, ID3D11DeviceContext, ID3D11ShaderResourceView, ID3D11Texture2D,
};
use windows::Win32::Graphics::Dxgi::Common::{DXGI_FORMAT_R8G8B8A8_UNORM, DXGI_SAMPLE_DESC};

use crate::layer_texture::LayerTexture;

/// The GPU textures of every layer, keyed by the owner slot and the owner's own texture id, so one mod never
/// reaches the textures of another. Render thread only. Holds at most `MAX_TEXTURES` entries; a new texture past
/// that is not created. The mods together hold at most `MAX_TEXTURES - MANAGER_RESERVED`, so the manager's panel
/// can always create its own. Entries leave when their owner frees them, when the owner's slot is freed, or on
/// `drop_all`.
pub struct TextureRegistry {
    // one map per owner byte, so freeing an owner drops its map and no id of one owner meets another's
    owners: Vec<HashMap<u64, LayerTexture>>,
    count: usize,
    limit_logged: bool,
    // the manager panel's refused upload was logged for this open
    manager_refusal_logged: bool,
}

impl TextureRegistry {
    pub const MAX_TEXTURES: usize = 8192;
    pub const MANAGER_RESERVED: usize = 64;
    const BYTES_PER_PIXEL: u32 = 4;
    const OWNER_SLOT_COUNT: u32 = 32;
    const OWNER_COUNT: usize = 256;

    pub fn new() -> Self {
        Self {
            owners: (0..Self::OWNER_COUNT).map(|_| HashMap::new()).collect(),
            count: 0,
            limit_logged: false,
            manager_refusal_logged: false,
        }
    }

    /// Creates, replaces or updates the textures `deltas` name for `owner`. The pixels of each delta are
    /// `bytes[byte_offset..]`; a delta whose pixels do not fit `bytes` is skipped. A full delta creates the
    /// texture or replaces it. A partial delta updates a rectangle of an existing texture, and is skipped when the
    /// texture does not exist or the rectangle is not inside it.
    pub fn apply(
        &mut self,
        device: &ID3D11Device,
        ctx: &ID3D11DeviceContext,
        owner: u8,
        deltas: &[AmmTextureDelta],
        bytes: &[u8],
    ) {
        for delta in deltas {
            let Some(pixels) = Self::pixels_of(bytes, delta) else {
                if owner == AmmHostLayer::MANAGER {
                    self.note_manager_refusal("delta pixels do not fit the mesh bytes", delta);
                }
                continue;
            };
            if delta.full != 0 {
                self.apply_full(device, owner, delta, pixels);
            } else {
                self.apply_partial(ctx, owner, delta, pixels);
            }
        }
    }

    /// Drops the textures `ids` of `owner`. Unknown ids are ignored.
    pub fn free(&mut self, owner: u8, ids: &[u64]) {
        let textures = &mut self.owners[usize::from(owner)];
        for id in ids {
            if textures.remove(id).is_some() {
                self.count -= 1;
            }
        }
    }

    /// Drops every texture of each owner whose bit is set in `owners` (bit N is slot N).
    pub fn free_owners(&mut self, owners: u32) {
        if owners == 0 {
            return;
        }
        for owner in 0..Self::OWNER_SLOT_COUNT {
            if owners & (1 << owner) == 0 {
                continue;
            }
            let textures = &mut self.owners[owner as usize];
            self.count -= textures.len();
            textures.clear();
        }
    }

    /// The view of `owner`'s texture `id`, or `None` when it does not exist.
    pub fn find(&self, owner: u8, id: u64) -> Option<&ID3D11ShaderResourceView> {
        self.owners[usize::from(owner)].get(&id).and_then(|texture| texture.view.as_ref())
    }

    /// The textures held, across every owner.
    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Drops every texture; the caller then tells the manager its textures are lost, so they come again.
    pub fn drop_all(&mut self) {
        for textures in self.owners.iter_mut() {
            textures.clear();
        }
        self.count = 0;
        self.limit_logged = false;
    }

    /// Re-arms the once-per-open log of a refused manager texture.
    pub fn manager_closed(&mut self) {
        self.manager_refusal_logged = false;
    }

    /// The view of `owner`'s texture `id` as the one-element slice a draw binds.
    pub(crate) fn bound_view(&self, owner: u8, id: u64) -> Option<&[Option<ID3D11ShaderResourceView>]> {
        self.owners[usize::from(owner)].get(&id).map(|texture| std::slice::from_ref(&texture.view))
    }

    fn pixels_of<'a>(bytes: &'a [u8], delta: &AmmTextureDelta) -> Option<&'a [u8]> {
        if delta.width == 0 || delta.height == 0 {
            return None;
        }
        let needed = u64::from(delta.width) * u64::from(delta.height) * u64::from(Self::BYTES_PER_PIXEL);
        let end = u64::from(delta.byte_offset) + u64::from(delta.byte_len);
        if u64::from(delta.byte_len) < needed || end > bytes.len() as u64 {
            return None;
        }
        let start = delta.byte_offset as usize;
        bytes.get(start..start + needed as usize)
    }

    // true when `owner` may add one more texture: the manager up to MAX_TEXTURES, the mods together up to
    // MAX_TEXTURES - MANAGER_RESERVED
    fn has_room(&self, owner: u8) -> bool {
        if owner == AmmHostLayer::MANAGER {
            return self.count < Self::MAX_TEXTURES;
        }
        let mod_count = self.count - self.owners[usize::from(AmmHostLayer::MANAGER)].len();
        mod_count < Self::MAX_TEXTURES - Self::MANAGER_RESERVED
    }

    // a full delta is create-or-replace: after the textures are lost the owner sends it again with the same id
    fn apply_full(&mut self, device: &ID3D11Device, owner: u8, delta: &AmmTextureDelta, pixels: &[u8]) {
        let exists = self.owners[usize::from(owner)].contains_key(&delta.id);
        if !exists && !self.has_room(owner) {
            if owner == AmmHostLayer::MANAGER {
                self.note_manager_refusal("texture registry is full", delta);
            }
            if !self.limit_logged {
                curia::warn!("texture registry is full, extra textures are not drawn", {
                    limit: Self::MAX_TEXTURES,
                    owner: owner
                });
            }
            self.limit_logged = true;
            return;
        }
        let Some(texture) = Self::create(device, delta, pixels) else {
            if owner == AmmHostLayer::MANAGER {
                self.note_manager_refusal("CreateTexture2D failed", delta);
            }
            curia::warn!("could not create a layer texture", {
                owner: owner,
                id: delta.id,
                width: delta.width,
                height: delta.height
            });
            return;
        };
        if self.owners[usize::from(owner)].insert(delta.id, texture).is_none() {
            self.count += 1;
        }
    }

    fn apply_partial(&mut self, ctx: &ID3D11DeviceContext, owner: u8, delta: &AmmTextureDelta, pixels: &[u8]) {
        let Some(texture) = self.owners[usize::from(owner)].get(&delta.id) else {
            return;
        };
        if u64::from(delta.x) + u64::from(delta.width) > u64::from(texture.width)
            || u64::from(delta.y) + u64::from(delta.height) > u64::from(texture.height)
        {
            return;
        }
        let area = D3D11_BOX {
            left: delta.x,
            top: delta.y,
            front: 0,
            right: delta.x + delta.width,
            bottom: delta.y + delta.height,
            back: 1,
        };
        // SAFETY: `pixels` holds `height` rows of `width * 4` bytes, as `pixels_of` checked, and the box is inside
        // the texture, as checked above
        unsafe {
            ctx.UpdateSubresource(
                &texture.texture,
                0,
                Some(&area),
                pixels.as_ptr().cast(),
                delta.width * Self::BYTES_PER_PIXEL,
                0,
            );
        }
    }

    fn create(device: &ID3D11Device, delta: &AmmTextureDelta, pixels: &[u8]) -> Option<LayerTexture> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: delta.width,
            Height: delta.height,
            MipLevels: 1,
            ArraySize: 1,
            Format: DXGI_FORMAT_R8G8B8A8_UNORM,
            SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_SHADER_RESOURCE.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
        };
        let data = D3D11_SUBRESOURCE_DATA {
            pSysMem: pixels.as_ptr().cast(),
            SysMemPitch: delta.width * Self::BYTES_PER_PIXEL,
            SysMemSlicePitch: 0,
        };
        let mut texture: Option<ID3D11Texture2D> = None;
        // SAFETY: desc and data live for the call, and `pixels` holds `height` rows of the given pitch
        unsafe { device.CreateTexture2D(&desc, Some(&data), Some(&mut texture)) }.ok()?;
        let texture = texture?;
        let mut view: Option<ID3D11ShaderResourceView> = None;
        // SAFETY: the texture is live and was made with D3D11_BIND_SHADER_RESOURCE
        unsafe { device.CreateShaderResourceView(&texture, None, Some(&mut view)) }.ok()?;
        view.as_ref()?;
        Some(LayerTexture { texture, view, width: delta.width, height: delta.height })
    }

    // one line per panel open when an upload for the manager layer fails or is refused
    fn note_manager_refusal(&mut self, reason: &'static str, delta: &AmmTextureDelta) {
        if self.manager_refusal_logged {
            return;
        }
        self.manager_refusal_logged = true;
        curia::debug!("manager texture refused", {
            reason: reason,
            id: delta.id,
            width: delta.width,
            height: delta.height
        });
    }
}

impl Default for TextureRegistry {
    fn default() -> Self {
        Self::new()
    }
}
