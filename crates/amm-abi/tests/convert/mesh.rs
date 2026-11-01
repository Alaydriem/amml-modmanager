use amm_abi::types::{AmmDrawCmd, AmmMeshView, AmmStatus, AmmTextureDelta};
use amm_core::structs::mesh::{DrawCmd, MeshBuffers, SamplerKind, TextureDelta, Vertex};

#[test]
fn mesh_view_borrows_the_buffers_without_copying() {
    let mut buffers = MeshBuffers::new();
    buffers.vertices.push(Vertex { pos: [1.0, 2.0], uv: [0.0, 1.0], color: [1, 2, 3, 4] });
    buffers.vertices.push(Vertex { pos: [3.0, 4.0], uv: [1.0, 0.0], color: [5, 6, 7, 8] });
    buffers.indices.extend([0, 1, 0]);
    buffers.cmds.push(DrawCmd {
        texture: 9,
        index_offset: 0,
        index_count: 3,
        clip: [0.0, 0.0, 10.0, 10.0],
        shader: DrawCmd::SHADER_USER,
        sampler: SamplerKind::Nearest,
    });
    buffers.bytes.extend([1, 2, 3]);
    buffers.freed.push(5);

    let view = AmmMeshView::from(&buffers);

    assert_eq!(view.vertex_count, 2);
    assert_eq!(view.index_count, 3);
    assert_eq!(view.cmd_count, 1);
    assert_eq!(view.delta_count, 0);
    assert_eq!(view.byte_count, 3);
    assert_eq!(view.freed_count, 1);
    assert_eq!(view.vertices as usize, buffers.vertices.as_ptr() as usize);
    assert_eq!(view.indices, buffers.indices.as_ptr());
    assert_eq!(view.cmds as usize, buffers.cmds.as_ptr() as usize);
    assert_eq!(view.bytes, buffers.bytes.as_ptr());
    assert_eq!(view.freed, buffers.freed.as_ptr());
}

#[test]
fn an_empty_mesh_reports_zero_counts() {
    let buffers = MeshBuffers::new();
    let view = AmmMeshView::from(&buffers);
    assert_eq!(view.vertex_count, 0);
    assert_eq!(view.cmd_count, 0);
    assert_eq!(view.byte_count, 0);
}

fn abi_cmd(sampler: u8) -> AmmDrawCmd {
    AmmDrawCmd { texture: 1, index_offset: 2, index_count: 3, clip: [0.0; 4], shader: 1, sampler }
}

fn abi_delta(sampler: u8, full: u8) -> AmmTextureDelta {
    AmmTextureDelta { id: 1, sampler, full, x: 0, y: 0, width: 1, height: 1, byte_offset: 0, byte_len: 4 }
}

#[test]
fn a_draw_cmd_with_a_bad_sampler_byte_is_rejected() {
    assert_eq!(DrawCmd::try_from(&abi_cmd(1)).map(|cmd| cmd.sampler), Ok(SamplerKind::Nearest));
    assert_eq!(DrawCmd::try_from(&abi_cmd(2)), Err(AmmStatus::InvalidArgument));
}

#[test]
fn a_texture_delta_with_a_bad_sampler_or_full_byte_is_rejected() {
    let good = TextureDelta::try_from(&abi_delta(0, 1));
    assert_eq!(good.map(|delta| (delta.sampler, delta.full)), Ok((SamplerKind::Linear, true)));
    assert_eq!(TextureDelta::try_from(&abi_delta(9, 0)), Err(AmmStatus::InvalidArgument));
    assert_eq!(TextureDelta::try_from(&abi_delta(0, 2)), Err(AmmStatus::InvalidArgument));
}
