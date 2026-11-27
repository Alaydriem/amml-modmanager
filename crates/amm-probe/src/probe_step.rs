/// Where the probe is in its checks. Each step ends when its line is logged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProbeStep {
    /// Waits for the link to connect.
    #[default]
    Link,
    /// Waits for the magenta square to be drawn for `ProbeLayer::TARGET_FRAMES` frames.
    Layer,
    /// Changes `probe.count` and waits for `SettingsChanged`.
    Setting,
    /// Takes the HUD lease.
    Hud,
    /// Holds W through the input lease for `ProbeChecks::HOLD_FRAMES` frames.
    KeyHold,
    /// Holds a forward movement until the movement event shows it.
    Move,
    /// Injects a 90 degree turn and waits for the look event.
    Turn,
    /// Injects one wheel notch and one left click.
    Mouse,
    /// Holds a FULL input lease for `ProbeChecks::FULL_FRAMES` frames.
    FullLease,
    /// Asks for both screenshots and waits for both files.
    Screenshot,
    /// Every check passed; holds W for the focus-loss and world-leave checks.
    Done,
}
