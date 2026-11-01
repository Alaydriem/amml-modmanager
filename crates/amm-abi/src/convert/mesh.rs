use amm_core::structs::mesh::{DrawCmd, MeshBuffers, TextureDelta};

use crate::types::{AmmDrawCmd, AmmMeshView, AmmSamplerKind, AmmStatus, AmmTextureDelta, AmmVertex};
use amm_core::structs::mesh::SamplerKind;

impl From<SamplerKind> for AmmSamplerKind {
    fn from(value: SamplerKind) -> Self {
        match value {
            SamplerKind::Linear => AmmSamplerKind::Linear,
            SamplerKind::Nearest => AmmSamplerKind::Nearest,
        }
    }
}

impl From<AmmSamplerKind> for SamplerKind {
    fn from(value: AmmSamplerKind) -> Self {
        match value {
            AmmSamplerKind::Linear => SamplerKind::Linear,
            AmmSamplerKind::Nearest => SamplerKind::Nearest,
        }
    }
}

impl From<&MeshBuffers> for AmmMeshView {
    fn from(value: &MeshBuffers) -> Self {
        let vertices: &[AmmVertex] = bytemuck::cast_slice(&value.vertices);
        Self {
            vertices: vertices.as_ptr(),
            vertex_count: vertices.len(),
            indices: value.indices.as_ptr(),
            index_count: value.indices.len(),
            // DrawCmd and TextureDelta are #[repr(C)] with the same field order and types as their mirrors,
            // which tests/types/layout.rs checks.
            cmds: value.cmds.as_ptr().cast::<AmmDrawCmd>(),
            cmd_count: value.cmds.len(),
            deltas: value.deltas.as_ptr().cast::<AmmTextureDelta>(),
            delta_count: value.deltas.len(),
            bytes: value.bytes.as_ptr(),
            byte_count: value.bytes.len(),
            freed: value.freed.as_ptr(),
            freed_count: value.freed.len(),
        }
    }
}

impl TryFrom<u8> for AmmSamplerKind {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmSamplerKind::Linear),
            1 => Ok(AmmSamplerKind::Nearest),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}

impl TryFrom<&AmmDrawCmd> for DrawCmd {
    type Error = AmmStatus;

    fn try_from(value: &AmmDrawCmd) -> Result<Self, Self::Error> {
        Ok(Self {
            texture: value.texture,
            index_offset: value.index_offset,
            index_count: value.index_count,
            clip: value.clip,
            shader: value.shader,
            sampler: AmmSamplerKind::try_from(value.sampler)?.into(),
        })
    }
}

impl TryFrom<&AmmTextureDelta> for TextureDelta {
    type Error = AmmStatus;

    fn try_from(value: &AmmTextureDelta) -> Result<Self, Self::Error> {
        let full = match value.full {
            0 => false,
            1 => true,
            _ => return Err(AmmStatus::InvalidArgument),
        };
        Ok(Self {
            id: value.id,
            sampler: AmmSamplerKind::try_from(value.sampler)?.into(),
            full,
            x: value.x,
            y: value.y,
            width: value.width,
            height: value.height,
            byte_offset: value.byte_offset,
            byte_len: value.byte_len,
        })
    }
}
