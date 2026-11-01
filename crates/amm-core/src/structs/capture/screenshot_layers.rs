/// What one screenshot shows. `GameOnly` is taken after the game has drawn and before any mod layer, toast or the
/// panel. `WithModLayers` is taken after the last mod layer and before the manager's layer (the panel and the
/// toasts), so it shows the layers of all mods, as the player sees them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenshotLayers {
    GameOnly,
    WithModLayers,
}
