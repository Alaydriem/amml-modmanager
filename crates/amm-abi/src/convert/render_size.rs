use amm_core::structs::render::{RenderSize, RenderSizeState};

use amm_core::traits::AbiSized;

use crate::types::{AmmRenderSize, AmmRenderSizeState};

impl From<RenderSize> for AmmRenderSize {
    fn from(value: RenderSize) -> Self {
        Self { width: value.width, height: value.height }
    }
}

impl From<AmmRenderSize> for RenderSize {
    fn from(value: AmmRenderSize) -> Self {
        RenderSize::new(value.width, value.height)
    }
}

impl From<RenderSizeState> for AmmRenderSizeState {
    fn from(value: RenderSizeState) -> Self {
        Self {
            wanted: value.wanted.map_or(AmmRenderSize { width: 0, height: 0 }, AmmRenderSize::from),
            actual: value.actual.into(),
            format: value.format,
            ..Self::blank()
        }
    }
}

impl From<AmmRenderSizeState> for RenderSizeState {
    fn from(value: AmmRenderSizeState) -> Self {
        let wanted = RenderSize::from(value.wanted);
        Self { wanted: (!wanted.is_empty()).then_some(wanted), actual: value.actual.into(), format: value.format }
    }
}
