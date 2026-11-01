/// What a screenshot shows, as the `layers` byte of `request_screenshot_layers`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmmScreenshotLayers {
    /// Taken after the game has drawn and before any mod layer, toast or the panel.
    GameOnly = 0,
    /// Taken after the last mod layer and before the manager's panel and toasts: the layers of all mods show.
    WithModLayers = 1,
}
