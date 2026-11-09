use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use amm_core::structs::input::{PointerButton as FakeButton, PointerOp};
use amm_overlay::PointerButton;
use amml_modmanager::game_hooks::feed;

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::{Call, RecordingCalls};

// action, data, x, y, dx, dy, motionless as the game's original got them
type Feed = (u8, i8, i16, i16, i16, i16, u8);

static GAME_FEED: Mutex<Vec<Feed>> = Mutex::new(Vec::new());

#[allow(clippy::too_many_arguments)]
extern "C" fn game_feed(_this: usize, action: u8, data: i8, x: i16, y: i16, dx: i16, dy: i16, motionless: u8) {
    GAME_FEED.lock().unwrap().push((action, data, x, y, dx, dy, motionless));
}

#[test]
fn a_consumed_button_is_cancelled_and_a_free_one_passes_with_the_viewport_point() {
    let (_, calls) = InstalledRuntime::install();
    feed::GUARD.original().store(game_feed as *mut c_void, Ordering::SeqCst);
    calls.consumed_buttons.lock().unwrap().push(PointerButton::Left);
    *calls.game_pointer.lock().unwrap() = Some((50, 60));

    feed::detour(0x8000, 1, 1, 10, 20, 0, 0, 0);
    feed::detour(0x8000, 2, 1, 10, 20, 0, 0, 0);

    assert_eq!(*GAME_FEED.lock().unwrap(), [(2, 1, 50, 60, 0, 0, 0)]);
}

#[test]
fn a_consumed_movement_is_cancelled() {
    let (_, calls) = InstalledRuntime::install();
    feed::GUARD.original().store(game_feed as *mut c_void, Ordering::SeqCst);
    calls.consume_deltas.store(true, Ordering::SeqCst);

    feed::detour(0x8000, 0, 0, 10, 20, 3, -2, 0);

    assert!(GAME_FEED.lock().unwrap().is_empty());
}

#[test]
fn a_real_mouse_event_sets_where_fake_buttons_go() {
    let (state, calls) = InstalledRuntime::install();
    feed::GUARD.original().store(game_feed as *mut c_void, Ordering::SeqCst);
    *calls.game_pointer.lock().unwrap() = Some((50, 60));
    calls.queue_fake_pointer(&[PointerOp::Button { button: FakeButton::Right, down: true }]);

    feed::detour(0x8800, 0, 0, 10, 20, 0, 0, 0);
    state.fake_pointer().flush(calls, RecordingCalls::CLIENT);

    assert!(calls.take().contains(&Call::GameMouse { device: 0x8800, action: 2, data: 1, x: 50, y: 60 }));
}
