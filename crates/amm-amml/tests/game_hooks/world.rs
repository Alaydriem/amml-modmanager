use std::ffi::c_void;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};

use amml_modmanager::game_hooks::{level_exit, player_loaded};
use amml_modmanager::services::{ManagerRuntime, ManagerState};

use crate::support::host_dirs::HostDirs;
use crate::support::recording_calls::{Call, RecordingCalls};

static CALLS: OnceLock<&'static RecordingCalls> = OnceLock::new();
// what the host had heard when each original ran
static SEEN_BY_ORIGINAL: Mutex<Vec<Vec<Call>>> = Mutex::new(Vec::new());

extern "C" fn game_player_loaded(_this: usize, _player: usize) {
    SEEN_BY_ORIGINAL.lock().unwrap().push(CALLS.get().unwrap().seen());
}

extern "C" fn game_level_exit(_this: usize) {
    SEEN_BY_ORIGINAL.lock().unwrap().push(CALLS.get().unwrap().seen());
}

fn install(dirs: &HostDirs) -> (&'static ManagerState, &'static RecordingCalls) {
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    let _ = CALLS.set(calls);
    player_loaded::GUARD.original().store(game_player_loaded as *mut c_void, Ordering::SeqCst);
    level_exit::GUARD.original().store(game_level_exit as *mut c_void, Ordering::SeqCst);
    ManagerRuntime::install(ManagerRuntime::new(state, calls, dirs.paths()));
    (state, calls)
}

#[test]
fn a_loaded_world_reaches_the_host_before_the_original() {
    let dirs = HostDirs::new();
    let (_, calls) = install(&dirs);

    player_loaded::detour(0x1000, 0x5000);

    assert_eq!(*SEEN_BY_ORIGINAL.lock().unwrap(), vec![vec![Call::HostWorld(true)]]);
    assert_eq!(calls.take(), vec![Call::HostWorld(true)]);
}

#[test]
fn leaving_a_world_reaches_the_host_before_the_original_and_forgets_the_client() {
    let dirs = HostDirs::new();
    let (state, calls) = install(&dirs);
    ManagerRuntime::get().unwrap().tick(0x1000);
    calls.take();

    level_exit::detour(0x1088);

    assert_eq!(*SEEN_BY_ORIGINAL.lock().unwrap(), vec![vec![Call::HostWorld(false)]]);
    assert_eq!(state.tick().client(), 0);
}
