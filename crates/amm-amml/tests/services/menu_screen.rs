use amml_modmanager::services::{ManagerState, MenuScreen};

use crate::support::recording_calls::{Call, RecordingCalls};

fn observe(name: Option<&str>) -> (u8, Vec<Call>) {
    let calls = RecordingCalls::leaked(ManagerState::leaked(), false);
    *calls.screen_name.lock().unwrap() = name.map(str::to_string);
    let menu = MenuScreen::new();
    menu.observe(calls, RecordingCalls::CLIENT);
    (menu.current(), calls.take())
}

#[test]
fn the_pause_screen_gives_2_and_its_string_is_freed_once() {
    assert_eq!(observe(Some("pause_screen")), (MenuScreen::PAUSE, vec![Call::ReleaseString]));
}

#[test]
fn the_start_screen_gives_1_and_any_other_screen_0() {
    assert_eq!(observe(Some("start_screen")).0, MenuScreen::START);
    assert_eq!(observe(Some("hud_screen")).0, MenuScreen::NONE);
    assert_eq!(observe(Some("a screen name longer than fifteen bytes")), (MenuScreen::NONE, vec![Call::ReleaseString]));
}

#[test]
fn without_the_entry_the_screen_is_0_and_nothing_is_freed() {
    assert_eq!(observe(None), (MenuScreen::NONE, Vec::new()));
}
