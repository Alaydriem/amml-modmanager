use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_abi::types::AmmCursorShape;
use amml_modmanager::game_hooks::grab_mouse;

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::{Call, RecordingCalls};

static GAME_GRABS: AtomicU32 = AtomicU32::new(0);

extern "C" fn game_grab(_this: usize) {
    GAME_GRABS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn the_game_grab_is_skipped_while_the_cursor_is_free_and_its_this_is_kept() {
    let (state, calls) = InstalledRuntime::install();
    grab_mouse::GUARD.original().store(game_grab as *mut c_void, Ordering::SeqCst);

    grab_mouse::detour(0x9000);
    assert_eq!(GAME_GRABS.load(Ordering::SeqCst), 1);

    state.frame().publish(state.latch(), true, AmmCursorShape::Default, false);
    grab_mouse::detour(0x9000);
    assert_eq!(GAME_GRABS.load(Ordering::SeqCst), 1);

    calls.set_in_play(true);
    state.grab().apply(calls, state.latch(), state.frame(), RecordingCalls::CLIENT);
    assert_eq!(calls.take(), vec![Call::ReleaseMouse(0x9000)], "the kept MinecraftGame is released");
}
