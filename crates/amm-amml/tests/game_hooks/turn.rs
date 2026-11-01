use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};

use amml_modmanager::game_hooks::apply_turn_delta;

use crate::support::installed_runtime::InstalledRuntime;

static GAME_TURNS: AtomicU32 = AtomicU32::new(0);

extern "C" fn game_turn(_this: usize, _delta: usize) {
    GAME_TURNS.fetch_add(1, Ordering::SeqCst);
}

#[test]
fn the_turn_is_dropped_while_the_host_blocks_it() {
    let (_, calls) = InstalledRuntime::install();
    apply_turn_delta::GUARD.original().store(game_turn as *mut c_void, Ordering::SeqCst);

    apply_turn_delta::detour(0x3000, 0x5000);
    calls.blocks_turn.store(true, Ordering::SeqCst);
    apply_turn_delta::detour(0x3000, 0x5000);

    assert_eq!(GAME_TURNS.load(Ordering::SeqCst), 1);
}
