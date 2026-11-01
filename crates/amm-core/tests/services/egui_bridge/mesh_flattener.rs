use amm_core::services::egui_bridge::MeshFlattener;
use amm_core::structs::frame::LayerKind;
use amm_core::structs::mesh::{DrawCmd, GamePicture, MeshBuffers, SamplerKind};
use egui::epaint::Vertex as EguiVertex;
use egui::epaint::textures::{TextureManager, TexturesDelta};
use egui::epaint::{ClippedPrimitive, Color32, ColorImage, ImageData, ImageDelta, Mesh, Primitive};
use egui::{Rect, TextureId, TextureOptions, pos2};

fn quad(texture_id: TextureId, clip: Rect, at: f32) -> ClippedPrimitive {
    let mut mesh = Mesh::with_texture(texture_id);
    let corners = [pos2(at, at), pos2(at + 4.0, at), pos2(at + 4.0, at + 4.0), pos2(at, at + 4.0)];
    for pos in corners {
        mesh.vertices.push(EguiVertex { pos, uv: pos2(0.0, 0.0), color: Color32::WHITE });
    }
    mesh.indices.extend([0, 1, 2, 0, 2, 3]);
    ClippedPrimitive { clip_rect: clip, primitive: Primitive::Mesh(mesh) }
}

fn big_clip() -> Rect {
    Rect::from_min_max(pos2(0.0, 0.0), pos2(100.0, 100.0))
}

#[test]
fn flattened_vertices_are_in_physical_pixels() {
    let mut out = MeshBuffers::new();
    let primitives = [quad(TextureId::Managed(0), big_clip(), 10.0)];

    MeshFlattener::flatten(&primitives, &TexturesDelta::default(), &TextureManager::default(), 1.25, &mut out);

    assert_eq!(out.vertices[0].pos, [12.5, 12.5]);
    assert_eq!(out.cmds[0].clip, [0.0, 0.0, 125.0, 125.0]);
    assert_eq!(out.vertex_count(), 4);
}

#[test]
fn empty_clip_rects_are_skipped() {
    let mut out = MeshBuffers::new();
    let empty = Rect::from_min_max(pos2(5.0, 5.0), pos2(5.0, 20.0));
    let primitives = [quad(TextureId::Managed(0), empty, 10.0), quad(TextureId::Managed(0), big_clip(), 10.0)];

    MeshFlattener::flatten(&primitives, &TexturesDelta::default(), &TextureManager::default(), 1.0, &mut out);

    assert_eq!(out.cmds.len(), 1);
    assert_eq!(out.vertex_count(), 4);
}

#[test]
fn second_mesh_indices_are_rebased_and_offsets_chain() {
    let mut out = MeshBuffers::new();
    let primitives = [quad(TextureId::Managed(0), big_clip(), 1.0), quad(TextureId::Managed(0), big_clip(), 2.0)];

    MeshFlattener::flatten(&primitives, &TexturesDelta::default(), &TextureManager::default(), 1.0, &mut out);

    assert_eq!(out.cmds[1].index_offset, 6);
    assert_eq!(&out.indices[6..9], &[4, 5, 6]);
}

#[test]
fn user_texture_keeps_sampler_bits() {
    let mut out = MeshBuffers::new();
    let primitives = [
        quad(MeshFlattener::user_texture(7, SamplerKind::Nearest), big_clip(), 1.0),
        quad(MeshFlattener::user_texture(7, SamplerKind::Linear), big_clip(), 1.0),
    ];

    MeshFlattener::flatten(&primitives, &TexturesDelta::default(), &TextureManager::default(), 1.0, &mut out);

    assert_eq!(out.cmds[0].sampler, SamplerKind::Nearest);
    assert_eq!(out.cmds[1].sampler, SamplerKind::Linear);
    assert_eq!(out.cmds[0].texture, 7 | DrawCmd::USER_TEXTURE_FLAG);
    assert_eq!(out.cmds[0].shader, DrawCmd::SHADER_USER);
}

#[test]
fn texture_delta_bytes_are_packed_rgba8_with_offsets() {
    let mut out = MeshBuffers::new();
    let full = ColorImage::new([2, 1], vec![Color32::from_rgba_premultiplied(1, 2, 3, 4); 2]);
    let patch = ColorImage::new([1, 1], vec![Color32::from_rgba_premultiplied(9, 8, 7, 6)]);
    let mut delta = TexturesDelta::default();
    let images = [
        ImageDelta::full(ImageData::Color(full.into()), TextureOptions::LINEAR),
        ImageDelta::partial([3, 4], ImageData::Color(patch.into()), TextureOptions::LINEAR),
    ];
    delta.set.insert(TextureId::Managed(1), images.into_iter().collect());
    delta.free.insert(TextureId::Managed(5));

    MeshFlattener::flatten(&[], &delta, &TextureManager::default(), 1.0, &mut out);

    assert_eq!(out.deltas.len(), 2);
    assert!(out.deltas[0].full);
    assert_eq!((out.deltas[0].byte_offset, out.deltas[0].byte_len), (0, 8));
    assert_eq!((out.deltas[1].byte_offset, out.deltas[1].byte_len), (8, 4));
    assert_eq!((out.deltas[1].x, out.deltas[1].y), (3, 4));
    assert_eq!(&out.bytes[8..12], &[9, 8, 7, 6]);
    assert_eq!(out.freed, vec![5]);
    delta.clear();
}

#[test]
fn overlay_layer_marks_managed_ids_and_keeps_user_ids() {
    let mut out = MeshBuffers::new();
    let image = ColorImage::new([1, 1], vec![Color32::WHITE]);
    let mut delta = TexturesDelta::default();
    let images = [ImageDelta::full(ImageData::Color(image.into()), TextureOptions::LINEAR)];
    delta.set.insert(TextureId::Managed(1), images.into_iter().collect());
    delta.free.insert(TextureId::Managed(5));
    let primitives = [
        quad(TextureId::Managed(1), big_clip(), 1.0),
        quad(MeshFlattener::user_texture(7, SamplerKind::Nearest), big_clip(), 1.0),
    ];

    MeshFlattener::flatten_layer(&primitives, &delta, &TextureManager::default(), 1.0, LayerKind::Overlay, &mut out);

    let bit = MeshFlattener::OVERLAY_TEXTURE_BIT;
    assert_eq!(out.deltas[0].id, 1 | bit);
    assert_eq!(out.freed, vec![5 | bit]);
    assert_eq!(out.cmds[0].texture, 1 | bit);
    assert_eq!(out.cmds[1].texture, 7 | DrawCmd::USER_TEXTURE_FLAG, "a user texture is the mod's own, on every layer");
    MeshFlattener::flatten_layer(&primitives, &delta, &TextureManager::default(), 1.0, LayerKind::Hud, &mut out);
    assert_eq!((out.deltas[0].id, out.freed[0], out.cmds[0].texture), (1, 5, 1));
    delta.clear();
}

#[test]
fn managed_texture_draws_and_uploads_with_its_own_magnification_filter() {
    let mut out = MeshBuffers::new();
    let mut textures = TextureManager::default();
    let pixel = || ImageData::Color(ColorImage::new([1, 1], vec![Color32::WHITE]).into());
    let nearest = textures.alloc("tile".into(), pixel(), TextureOptions::NEAREST);
    let linear = textures.alloc("atlas".into(), pixel(), TextureOptions::LINEAR);
    let mut delta = textures.take_delta();
    let primitives = [quad(nearest, big_clip(), 1.0), quad(linear, big_clip(), 1.0)];

    MeshFlattener::flatten(&primitives, &delta, &textures, 1.0, &mut out);

    assert_eq!(out.cmds[0].sampler, SamplerKind::Nearest);
    assert_eq!(out.cmds[1].sampler, SamplerKind::Linear);
    let sampler_of = |id: TextureId| {
        let TextureId::Managed(raw) = id else { unreachable!() };
        out.deltas.iter().find(|d| d.id == raw).map(|d| d.sampler)
    };
    assert_eq!(sampler_of(nearest), Some(SamplerKind::Nearest));
    assert_eq!(sampler_of(linear), Some(SamplerKind::Linear));
    delta.clear();
}

#[test]
fn the_game_picture_texture_is_drawn_with_its_reserved_host_id_its_own_shader_and_the_linear_sampler() {
    let mut out = MeshBuffers::new();
    let primitives = [quad(MeshFlattener::game_picture(), big_clip(), 1.0)];

    MeshFlattener::flatten(&primitives, &TexturesDelta::default(), &TextureManager::default(), 1.0, &mut out);

    assert_eq!(out.cmds[0].texture, GamePicture::HOST_TEXTURE);
    assert_eq!(out.cmds[0].shader, DrawCmd::SHADER_PICTURE);
    assert_eq!(out.cmds[0].sampler, SamplerKind::Linear);
    assert!(out.deltas.is_empty(), "the game picture is never uploaded by the mod");
}
