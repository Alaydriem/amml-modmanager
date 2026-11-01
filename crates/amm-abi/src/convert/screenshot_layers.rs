use amm_core::structs::capture::ScreenshotLayers;

use crate::types::{AmmScreenshotLayers, AmmStatus};

impl From<ScreenshotLayers> for AmmScreenshotLayers {
    fn from(value: ScreenshotLayers) -> Self {
        match value {
            ScreenshotLayers::GameOnly => AmmScreenshotLayers::GameOnly,
            ScreenshotLayers::WithModLayers => AmmScreenshotLayers::WithModLayers,
        }
    }
}

impl From<AmmScreenshotLayers> for ScreenshotLayers {
    fn from(value: AmmScreenshotLayers) -> Self {
        match value {
            AmmScreenshotLayers::GameOnly => ScreenshotLayers::GameOnly,
            AmmScreenshotLayers::WithModLayers => ScreenshotLayers::WithModLayers,
        }
    }
}

impl TryFrom<u8> for AmmScreenshotLayers {
    type Error = AmmStatus;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AmmScreenshotLayers::GameOnly),
            1 => Ok(AmmScreenshotLayers::WithModLayers),
            _ => Err(AmmStatus::InvalidArgument),
        }
    }
}
