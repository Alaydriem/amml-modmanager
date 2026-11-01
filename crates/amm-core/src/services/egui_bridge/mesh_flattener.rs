use egui::epaint::textures::{TextureManager, TexturesDelta};
use egui::epaint::{ImageData, Primitive};
use egui::{ClippedPrimitive, TextureFilter, TextureId, TextureOptions};

use crate::structs::frame::LayerKind;
use crate::structs::mesh::{DrawCmd, GamePicture, MeshBuffers, SamplerKind, TextureDelta, Vertex};

/// Fills a `MeshBuffers` from egui's tessellated meshes and texture changes, in physical pixels.
///
/// Texture ids sent to the host are 64 bits:
/// - bit 63, `DrawCmd::USER_TEXTURE_FLAG`, marks a user texture. Its low 32 bits are the mod's own id, and bit 32
///   asks for the linear sampler. A user id is the same on every layer.
/// - bit 62, `OVERLAY_TEXTURE_BIT`, marks an egui-managed id of the Overlay layer's context. A mod has one egui
///   context per layer and each context numbers its managed textures from 0, so this bit keeps the two sets apart
///   in the host's per-mod texture map. Hud ids leave the bit clear.
///
/// A managed texture is uploaded and drawn with the sampler of its own `TextureOptions::magnification`, which
/// `textures`, the manager of the context that made the pass, holds.
pub struct MeshFlattener;

impl MeshFlattener {
    /// Set on every managed texture id that the Overlay layer's context uploads, frees or draws with.
    pub const OVERLAY_TEXTURE_BIT: u64 = 1 << 62;
    const LINEAR_BIT: u64 = 1 << 32;
    const USER_ID_MASK: u64 = 0xFFFF_FFFF;

    /// Flattens one pass of a context that is the only one of its owner, with Hud ids. `out` is cleared first and
    /// keeps its capacity.
    pub fn flatten(
        primitives: &[ClippedPrimitive],
        delta: &TexturesDelta,
        textures: &TextureManager,
        ppp: f32,
        out: &mut MeshBuffers,
    ) {
        Self::flatten_layer(primitives, delta, textures, ppp, LayerKind::Hud, out);
    }

    /// Flattens one pass of the context of `layer`. Managed ids carry the layer bit. `out` is cleared first and
    /// keeps its capacity.
    pub fn flatten_layer(
        primitives: &[ClippedPrimitive],
        delta: &TexturesDelta,
        textures: &TextureManager,
        ppp: f32,
        layer: LayerKind,
        out: &mut MeshBuffers,
    ) {
        let tag = Self::managed_tag(layer);
        out.clear();
        Self::textures(delta, tag, out);
        Self::meshes(primitives, textures, ppp, tag, out);
    }

    /// The egui user texture id for a host texture id and sampler.
    pub fn user_texture(id: u32, sampler: SamplerKind) -> TextureId {
        let bit = if sampler == SamplerKind::Linear { Self::LINEAR_BIT } else { 0 };
        TextureId::User(u64::from(id) | bit)
    }

    /// The egui texture id of the live game picture (`GamePicture`), sampled linearly so it scales smoothly.
    pub fn game_picture() -> TextureId {
        Self::user_texture(GamePicture::USER_ID, SamplerKind::Linear)
    }

    fn managed_tag(layer: LayerKind) -> u64 {
        match layer {
            LayerKind::Hud => 0,
            LayerKind::Overlay => Self::OVERLAY_TEXTURE_BIT,
        }
    }

    fn textures(delta: &TexturesDelta, tag: u64, out: &mut MeshBuffers) {
        for (id, images) in &delta.set {
            let TextureId::Managed(texture) = *id else { continue };
            for image in images {
                let ImageData::Color(colour) = &image.image;
                let byte_offset = out.bytes.len();
                out.bytes.reserve(colour.pixels.len() * 4);
                for pixel in &colour.pixels {
                    out.bytes.extend_from_slice(&pixel.to_array());
                }
                let (x, y) = image.pos.map_or((0, 0), |[x, y]| (x as u32, y as u32));
                out.deltas.push(TextureDelta {
                    id: texture | tag,
                    sampler: Self::sampler(image.options),
                    full: image.pos.is_none(),
                    x,
                    y,
                    width: colour.size[0] as u32,
                    height: colour.size[1] as u32,
                    byte_offset: byte_offset as u32,
                    byte_len: (out.bytes.len() - byte_offset) as u32,
                });
            }
        }
        out.freed.extend(delta.free.iter().filter_map(|id| match *id {
            TextureId::Managed(texture) => Some(texture | tag),
            TextureId::User(_) => None,
        }));
    }

    fn meshes(primitives: &[ClippedPrimitive], textures: &TextureManager, ppp: f32, tag: u64, out: &mut MeshBuffers) {
        for primitive in primitives {
            let Primitive::Mesh(mesh) = &primitive.primitive else { continue };
            if mesh.indices.is_empty() {
                continue;
            }
            let rect = primitive.clip_rect;
            let clip = [rect.min.x * ppp, rect.min.y * ppp, rect.max.x * ppp, rect.max.y * ppp];
            if clip[2] <= clip[0] || clip[3] <= clip[1] {
                continue;
            }
            let (texture, shader, sampler) = match mesh.texture_id {
                TextureId::Managed(id) => {
                    let sampler =
                        textures.meta(mesh.texture_id).map_or(SamplerKind::Linear, |m| Self::sampler(m.options));
                    (id | tag, DrawCmd::SHADER_EGUI, sampler)
                }
                TextureId::User(raw) => {
                    let sampler = if raw & Self::LINEAR_BIT != 0 { SamplerKind::Linear } else { SamplerKind::Nearest };
                    let texture = (raw & Self::USER_ID_MASK) | DrawCmd::USER_TEXTURE_FLAG;
                    let shader = if texture == GamePicture::HOST_TEXTURE {
                        DrawCmd::SHADER_PICTURE
                    } else {
                        DrawCmd::SHADER_USER
                    };
                    (texture, shader, sampler)
                }
            };
            let base = out.vertices.len() as u32;
            out.cmds.push(DrawCmd {
                texture,
                index_offset: out.indices.len() as u32,
                index_count: mesh.indices.len() as u32,
                clip,
                shader,
                sampler,
            });
            out.indices.extend(mesh.indices.iter().map(|index| index + base));
            out.vertices.extend(mesh.vertices.iter().map(|v| Vertex {
                pos: [v.pos.x * ppp, v.pos.y * ppp],
                uv: [v.uv.x, v.uv.y],
                color: v.color.to_array(),
            }));
        }
    }

    fn sampler(options: TextureOptions) -> SamplerKind {
        match options.magnification {
            TextureFilter::Nearest => SamplerKind::Nearest,
            TextureFilter::Linear => SamplerKind::Linear,
        }
    }
}
