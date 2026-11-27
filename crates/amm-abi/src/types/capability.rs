/// A capability a mod can hold a manager lease on. Game input is not one: a mod takes it from the loader. The value 1
/// is unused (it was `Input` before 3.0), so an old value is never read as another capability.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmCapability {
    Camera = 0,
    Hud = 2,
    Overlay = 3,
    WorldDraw = 4,
    RenderSize = 5,
}
