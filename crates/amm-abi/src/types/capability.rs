/// A capability a mod can hold a lease on.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmCapability {
    Camera = 0,
    Input,
    Hud,
    Overlay,
    WorldDraw,
    RenderSize,
}
