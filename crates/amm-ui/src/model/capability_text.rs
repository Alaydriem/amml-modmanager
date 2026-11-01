use amm_core::structs::descriptor::Capability;

use crate::AmmMessage;

/// The names and one-line meanings of the capabilities, as the panel and the toasts show them.
pub struct CapabilityText;

impl CapabilityText {
    /// The name in a table, a toast or a clash title.
    pub fn name(cap: Capability) -> AmmMessage {
        match cap {
            Capability::Camera => AmmMessage::CapabilityCamera,
            Capability::Input => AmmMessage::CapabilityInput,
            Capability::Hud => AmmMessage::CapabilityHud,
            Capability::Overlay => AmmMessage::CapabilityOverlay,
            Capability::WorldDraw => AmmMessage::CapabilityWorldDraw,
            Capability::RenderSize => AmmMessage::CapabilityRenderSize,
        }
    }

    pub fn blurb(cap: Capability) -> AmmMessage {
        match cap {
            Capability::Camera => AmmMessage::CameraBlurb,
            Capability::Input => AmmMessage::InputBlurb,
            Capability::Hud => AmmMessage::HudBlurb,
            Capability::Overlay => AmmMessage::OverlayBlurb,
            Capability::WorldDraw => AmmMessage::WorldDrawBlurb,
            Capability::RenderSize => AmmMessage::RenderSizeBlurb,
        }
    }
}
