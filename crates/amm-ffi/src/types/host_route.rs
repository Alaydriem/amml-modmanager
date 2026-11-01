/// The answer to one input event: `consume` is true when the game must not see the event.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AmmHostRoute {
    pub consume: bool,
}

impl AmmHostRoute {
    /// The event reaches the game unchanged.
    pub(crate) const PASS: AmmHostRoute = AmmHostRoute { consume: false };
}
