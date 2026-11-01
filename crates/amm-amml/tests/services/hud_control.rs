use amm_abi::types::AmmCursorShape;
use amml_modmanager::services::{FrameState, HudControl, ManagerState, StopLatch};

use crate::support::recording_calls::{Call, RecordingCalls};

fn tick(calls: &RecordingCalls, hud: &HudControl, latch: &StopLatch, frame: &FrameState, hide: bool) {
    frame.publish(latch, false, AmmCursorShape::Default, hide);
    hud.apply(calls, latch, frame, RecordingCalls::CLIENT);
}

#[test]
fn the_players_setting_comes_back_after_hide_vanilla_hud_turns_off() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let (hud, latch, frame) = (HudControl::new(), StopLatch::new(), FrameState::new());

    tick(calls, &hud, &latch, &frame, true);
    tick(calls, &hud, &latch, &frame, true);
    tick(calls, &hud, &latch, &frame, false);

    assert_eq!(calls.take(), vec![Call::SetHideGui(true), Call::SetHideGui(false)]);
}

#[test]
fn a_player_who_hid_the_hud_keeps_it_hidden() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    calls.player_hides_hud.store(true, std::sync::atomic::Ordering::SeqCst);
    let (hud, latch, frame) = (HudControl::new(), StopLatch::new(), FrameState::new());

    tick(calls, &hud, &latch, &frame, true);
    tick(calls, &hud, &latch, &frame, false);

    assert_eq!(calls.take(), vec![Call::SetHideGui(true), Call::SetHideGui(true)]);
}

#[test]
fn without_options_the_hud_is_untouched() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    *calls.options.lock().unwrap() = None;
    let (hud, latch, frame) = (HudControl::new(), StopLatch::new(), FrameState::new());

    tick(calls, &hud, &latch, &frame, true);
    hud.restore(calls, RecordingCalls::CLIENT);

    assert!(calls.take().is_empty());
}

#[test]
fn restore_puts_the_saved_value_back_once() {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    let (hud, latch, frame) = (HudControl::new(), StopLatch::new(), FrameState::new());
    tick(calls, &hud, &latch, &frame, true);
    calls.take();

    hud.restore(calls, RecordingCalls::CLIENT);
    hud.restore(calls, RecordingCalls::CLIENT);

    assert_eq!(calls.take(), vec![Call::SetHideGui(false)]);
}
