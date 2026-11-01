use std::ffi::c_void;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

use amml_modmanager::game_hooks::client_update;
use amml_modmanager::services::{ManagerRuntime, ManagerState};

use crate::support::host_dirs::HostDirs;
use crate::support::recording_calls::{Call, RecordingCalls};

const EXTRA: u64 = 0xDEAD_BEEF_0000_0001;
static ORIGINAL_CALLS: AtomicU32 = AtomicU32::new(0);
static CALLS: OnceLock<&'static RecordingCalls> = OnceLock::new();
// the third argument (r8 on 26.60) the original received
static ORIGINAL_EXTRA: AtomicU64 = AtomicU64::new(0);
// the original ran while the host had not ticked yet
static ORIGINAL_FIRST: AtomicBool = AtomicBool::new(false);

extern "C" fn game_update(_this: usize, init_finished: u8, extra: u64) -> u8 {
    ORIGINAL_EXTRA.store(extra, Ordering::SeqCst);
    ORIGINAL_CALLS.fetch_add(1, Ordering::SeqCst);
    if let Some(calls) = CALLS.get() {
        let ticked = calls.seen().iter().any(|call| matches!(call, Call::HostTick(_)));
        ORIGINAL_FIRST.store(!ticked, Ordering::SeqCst);
    }
    init_finished
}

fn install(calls: &'static RecordingCalls, state: &'static ManagerState, dirs: &HostDirs) -> &'static ManagerRuntime {
    let _ = CALLS.set(calls);
    client_update::GUARD.original().store(game_update as *mut c_void, Ordering::SeqCst);
    ManagerRuntime::install(ManagerRuntime::new(state, calls, dirs.paths()))
}

#[test]
fn a_panic_in_the_tick_calls_the_original_once_and_switches_the_manager_off() {
    let dirs = HostDirs::new();
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    calls.panic_on_tick.store(true, Ordering::SeqCst);
    let runtime = install(calls, state, &dirs);

    assert_eq!(client_update::detour(0x1000, 1, EXTRA), 1);

    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), 1);
    assert_eq!(ORIGINAL_EXTRA.load(Ordering::SeqCst), EXTRA, "the third argument reaches the original unchanged");
    assert!(runtime.is_off());
    assert!(!state.tick().game_ready());

    assert_eq!(client_update::detour(0x1000, 1, EXTRA), 1);
    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), 2);
    let ticks = calls.take().into_iter().filter(|call| matches!(call, Call::HostTick(_))).count();
    assert_eq!(ticks, 1, "after the panic the detour only calls the original");
}

#[test]
fn a_tick_runs_after_the_original_and_readies_the_overlay() {
    let dirs = HostDirs::new();
    let state = ManagerState::leaked();
    let calls = RecordingCalls::leaked(state, false);
    calls.set_now(42);
    install(calls, state, &dirs);

    assert_eq!(client_update::detour(0x2000, 0, EXTRA), 0);

    assert_eq!(ORIGINAL_CALLS.load(Ordering::SeqCst), 1);
    assert!(ORIGINAL_FIRST.load(Ordering::SeqCst), "the original runs before the host tick");
    assert_eq!(calls.take(), vec![Call::HostTick(42)]);
    assert!(state.tick().game_ready());
    assert_eq!(state.tick().client(), 0x2000);
    assert_eq!(ORIGINAL_EXTRA.load(Ordering::SeqCst), EXTRA, "the third argument reaches the original unchanged");
}
