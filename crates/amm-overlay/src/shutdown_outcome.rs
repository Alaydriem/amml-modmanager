/// How `Overlay::shutdown` ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownOutcome {
    /// No thread was left in a detour; the device, the targets and the layer textures were released.
    Drained,
    /// A thread stayed in a detour past the timeout. Nothing was released, because that thread can still use it;
    /// the overlay cannot be installed again in this process.
    Leaked,
}
