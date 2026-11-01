use amm_abi::types::AmmCursorShape;
use amml_modmanager::services::{FrameState, ManagerState, MouseGrab, StopLatch};

use crate::support::recording_calls::{Call, RecordingCalls};

const GAME: usize = 0x9000;

struct Setup {
    calls: &'static RecordingCalls,
    latch: StopLatch,
    frame: FrameState,
    grab: MouseGrab,
}

impl Setup {
    fn new() -> Self {
        let grab = MouseGrab::new();
        grab.keep_game(GAME);
        Self {
            calls: RecordingCalls::leaked(ManagerState::leaked(), false),
            latch: StopLatch::new(),
            frame: FrameState::new(),
            grab,
        }
    }

    fn tick_with_cursor_free(&self, free: bool) {
        self.frame.publish(&self.latch, free, AmmCursorShape::Default, false);
        self.grab.apply(self.calls, &self.latch, &self.frame, RecordingCalls::CLIENT);
    }
}

#[test]
fn a_cursor_change_in_a_menu_releases_nothing_and_the_same_change_in_play_does() {
    let setup = Setup::new();
    setup.calls.set_in_play(false);

    setup.tick_with_cursor_free(true);
    assert!(setup.calls.take().is_empty());

    setup.calls.set_in_play(true);
    setup.tick_with_cursor_free(true);
    assert_eq!(setup.calls.take(), vec![Call::ReleaseMouse(GAME)]);

    setup.tick_with_cursor_free(true);
    assert!(setup.calls.take().is_empty(), "one release per change");

    setup.tick_with_cursor_free(false);
    assert_eq!(setup.calls.take(), vec![Call::GrabMouse { game: GAME, latch_raised: false }]);
}

#[test]
fn nothing_is_applied_before_the_game_grabbed_once_or_while_the_latch_is_raised() {
    let setup = Setup::new();
    setup.calls.set_in_play(true);
    let unseen = MouseGrab::new();
    setup.frame.publish(&setup.latch, true, AmmCursorShape::Default, false);
    unseen.apply(setup.calls, &setup.latch, &setup.frame, RecordingCalls::CLIENT);
    setup.latch.raise();
    setup.grab.apply(setup.calls, &setup.latch, &setup.frame, RecordingCalls::CLIENT);

    assert!(setup.calls.take().is_empty());
}

#[test]
fn the_game_grab_is_skipped_only_while_the_cursor_is_free() {
    let latch = StopLatch::new();
    let frame = FrameState::new();
    assert!(!MouseGrab::blocks_grab(&frame));
    frame.publish(&latch, true, AmmCursorShape::Hand, false);
    assert!(MouseGrab::blocks_grab(&frame));
}

#[test]
fn restore_grabs_once_when_the_manager_had_freed_the_cursor() {
    let setup = Setup::new();
    setup.calls.set_in_play(true);
    setup.tick_with_cursor_free(true);
    setup.calls.take();

    setup.grab.restore(setup.calls, RecordingCalls::CLIENT, true);
    setup.grab.restore(setup.calls, RecordingCalls::CLIENT, true);

    assert_eq!(setup.calls.take(), vec![Call::GrabMouse { game: GAME, latch_raised: false }]);
}
