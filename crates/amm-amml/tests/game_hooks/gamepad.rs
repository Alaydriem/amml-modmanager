use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use amml_modmanager::game_hooks::gamepad_refresh;

use crate::support::installed_runtime::InstalledRuntime;

static GAME_POLLS: AtomicU32 = AtomicU32::new(0);

extern "C" fn game_refresh(_this: usize) {
    GAME_POLLS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn gamepad_input_is_dropped_under_full_block() {
    let (_, calls) = InstalledRuntime::install();
    gamepad_refresh::GUARD.original().store(game_refresh as *mut c_void, Ordering::SeqCst);
    calls.blocks_gamepad.store(true, Ordering::SeqCst);

    gamepad_refresh::detour(0x3000);

    assert_eq!(GAME_POLLS.load(Ordering::SeqCst), 0);
}

#[test]
fn gamepad_input_passes_without_full_block() {
    let (_, calls) = InstalledRuntime::install();
    gamepad_refresh::GUARD.original().store(game_refresh as *mut c_void, Ordering::SeqCst);
    calls.blocks_gamepad.store(false, Ordering::SeqCst);

    gamepad_refresh::detour(0x3000);

    assert_eq!(GAME_POLLS.load(Ordering::SeqCst), 1);
}
