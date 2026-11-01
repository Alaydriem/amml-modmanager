/// What a `PaceWindow` counted: the Presents, the time spent inside them and the GPU readbacks of the window.
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct PaceSummary {
    pub presents: u32,
    /// from the start of the window to its last Present
    pub elapsed_ms: u64,
    pub per_second: f64,
    pub present_ms_mean: f64,
    pub present_ms_max: f64,
    /// the longest gap between two Presents of the window
    pub interval_ms_max: f64,
    /// Presents that returned `DXGI_STATUS_OCCLUDED`
    pub occluded: u32,
    /// Presents made while the game window was the foreground window
    pub foreground: u32,
    /// Presents made while no game window was bound: neither foreground nor background
    pub unbound: u32,
    pub readbacks: u32,
    pub readback_ms_mean: f64,
    pub readback_ms_max: f64,
}
