/// The size message that gives the game window its own size back after a forced render size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderRestore {
    /// The game window, as a handle value.
    pub window: usize,
    /// The client width the game gets back.
    pub width: u32,
    /// The client height the game gets back.
    pub height: u32,
}
