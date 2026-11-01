/// The game hooks by the step of disable that removes them, in that order: input, then the mouse grab and the
/// look turn, then the world events and the tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookGroup {
    Input,
    Grab,
    Turn,
    World,
    Tick,
}
