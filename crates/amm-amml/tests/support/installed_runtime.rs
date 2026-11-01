use amml_modmanager::services::{ManagerRuntime, ManagerState};

use super::host_dirs::HostDirs;
use super::recording_calls::RecordingCalls;

/// The process runtime the detours reach, over a recording `GameCalls`. Each test runs in its own process, so
/// each test installs its own.
pub struct InstalledRuntime;

impl InstalledRuntime {
    pub fn install() -> (&'static ManagerState, &'static RecordingCalls) {
        let dirs = Box::leak(Box::new(HostDirs::new()));
        let state = ManagerState::leaked();
        let calls = RecordingCalls::leaked(state, false);
        ManagerRuntime::install(ManagerRuntime::new(state, calls, dirs.paths()));
        (state, calls)
    }
}
