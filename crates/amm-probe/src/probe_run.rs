use std::path::PathBuf;

use amml_api::ShotId;

use crate::ProbeStep;

/// What the probe has done and seen so far. `ProbeChecks` keeps it behind one lock: the tick moves `step`, and the
/// input listeners record what the loader raised.
#[derive(Debug, Default)]
pub struct ProbeRun {
    pub step: ProbeStep,
    /// Ticks since the step began.
    pub ticks: u32,
    /// Whether the step has done its first action.
    pub entered: bool,
    pub linked: bool,
    pub joined: bool,
    /// The game read the player's movement since the world join: the world is in play, past its loading screen.
    pub in_play: bool,
    /// Ticks in play before the key hold presses W.
    pub settle_ticks: u32,
    pub settings_changed: bool,
    /// A forward movement showed in the movement event during the key hold.
    pub moved: bool,
    pub move_seen: bool,
    /// The yaw of the injected look events of this mod, in degrees.
    pub yaw: f32,
    pub wheel_notches: Option<f32>,
    pub click_seen: bool,
    pub gamepad_skipped: u32,
    pub look_blocked: bool,
    /// The screenshots asked for and not written yet: id, layers name and path.
    pub shots: Vec<(ShotId, &'static str, PathBuf)>,
    pub shots_asked: bool,
    /// A hold of W that a focus loss or a world leave let go and the probe must start again.
    pub rehold: bool,
    /// The world was left and the probe waits for the release event, or for a rejoin.
    pub left: bool,
    /// The world was left after `probe ok` and has not been joined again.
    pub has_left: bool,
    pub left_ticks: u32,
    /// A W press waits for the loader's key feed, which is not ready until a real key has shown its thread.
    pub key_waiting: bool,
}

impl ProbeRun {
    /// Moves to `step` and forgets what the old step did.
    pub fn enter(&mut self, step: ProbeStep) {
        self.step = step;
        self.ticks = 0;
        self.entered = false;
    }
}
