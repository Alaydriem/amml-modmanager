use amm_core::structs::descriptor::Capability;

use crate::types::{AmmCapability, AmmStatus};

impl TryFrom<Capability> for AmmCapability {
    type Error = AmmStatus;

    /// `Input` is not a manager lease, so it has no ABI value: `InvalidArgument`.
    fn try_from(value: Capability) -> Result<Self, Self::Error> {
        match value {
            Capability::Camera => Ok(AmmCapability::Camera),
            Capability::Hud => Ok(AmmCapability::Hud),
            Capability::Overlay => Ok(AmmCapability::Overlay),
            Capability::WorldDraw => Ok(AmmCapability::WorldDraw),
            Capability::RenderSize => Ok(AmmCapability::RenderSize),
            Capability::Input => Err(AmmStatus::InvalidArgument),
        }
    }
}

impl From<AmmCapability> for Capability {
    fn from(value: AmmCapability) -> Self {
        match value {
            AmmCapability::Camera => Capability::Camera,
            AmmCapability::Hud => Capability::Hud,
            AmmCapability::Overlay => Capability::Overlay,
            AmmCapability::WorldDraw => Capability::WorldDraw,
            AmmCapability::RenderSize => Capability::RenderSize,
        }
    }
}

impl TryFrom<u8> for AmmCapability {
    type Error = AmmStatus;

    /// The unused value 1 (`Input` before 3.0) and any value above 5 are `InvalidArgument`.
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmCapability::Camera),
            2 => Ok(AmmCapability::Hud),
            3 => Ok(AmmCapability::Overlay),
            4 => Ok(AmmCapability::WorldDraw),
            5 => Ok(AmmCapability::RenderSize),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
