use amm_core::structs::descriptor::Capability;

use crate::types::{AmmCapability, AmmStatus};

impl From<Capability> for AmmCapability {
    fn from(value: Capability) -> Self {
        match value {
            Capability::Camera => AmmCapability::Camera,
            Capability::Input => AmmCapability::Input,
            Capability::Hud => AmmCapability::Hud,
            Capability::Overlay => AmmCapability::Overlay,
            Capability::WorldDraw => AmmCapability::WorldDraw,
            Capability::RenderSize => AmmCapability::RenderSize,
        }
    }
}

impl From<AmmCapability> for Capability {
    fn from(value: AmmCapability) -> Self {
        match value {
            AmmCapability::Camera => Capability::Camera,
            AmmCapability::Input => Capability::Input,
            AmmCapability::Hud => Capability::Hud,
            AmmCapability::Overlay => Capability::Overlay,
            AmmCapability::WorldDraw => Capability::WorldDraw,
            AmmCapability::RenderSize => Capability::RenderSize,
        }
    }
}

impl TryFrom<u8> for AmmCapability {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmCapability::Camera),
            1 => Ok(AmmCapability::Input),
            2 => Ok(AmmCapability::Hud),
            3 => Ok(AmmCapability::Overlay),
            4 => Ok(AmmCapability::WorldDraw),
            5 => Ok(AmmCapability::RenderSize),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
