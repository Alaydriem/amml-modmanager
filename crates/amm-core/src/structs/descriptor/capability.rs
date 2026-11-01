use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize)]
pub enum Capability {
    Camera,
    Input,
    Hud,
    Overlay,
    WorldDraw,
    RenderSize,
}

impl Capability {
    pub const ALL: [Capability; 6] = [
        Capability::Camera,
        Capability::Input,
        Capability::Hud,
        Capability::Overlay,
        Capability::WorldDraw,
        Capability::RenderSize,
    ];

    /// The name of the capability as a descriptor writes it, which `from_name` reads back.
    pub fn name(&self) -> &'static str {
        match self {
            Capability::Camera => "Camera",
            Capability::Input => "Input",
            Capability::Hud => "Hud",
            Capability::Overlay => "Overlay",
            Capability::WorldDraw => "WorldDraw",
            Capability::RenderSize => "RenderSize",
        }
    }

    /// The capability with this exact name, as written in a descriptor.
    pub fn from_name(name: &str) -> Option<Capability> {
        match name {
            "Camera" => Some(Capability::Camera),
            "Input" => Some(Capability::Input),
            "Hud" => Some(Capability::Hud),
            "Overlay" => Some(Capability::Overlay),
            "WorldDraw" => Some(Capability::WorldDraw),
            "RenderSize" => Some(Capability::RenderSize),
            _ => None,
        }
    }
}
