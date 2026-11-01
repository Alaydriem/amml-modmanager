use std::path::Path;

use amm_abi::types::{AmmApi, AmmStatus};
use amm_client::test_support::{FakeTable, SettingCall};
use amm_core::errors::LinkError;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::manager::TextureGrant;
use amm_core::structs::menu::{MenuEntryId, MenuScreens};
use amm_core::structs::registry::ModIndex;

#[test]
fn acquire_answers_the_outcome_set_for_each_capability_and_grants_the_rest() {
    let link = FakeTable::link_without_frame();
    FakeTable::set_acquire_outcome(Capability::WorldDraw, AcquireOutcome::Denied { holder: ModIndex(2) });
    FakeTable::set_acquire_outcome(Capability::Camera, AcquireOutcome::Preempted);
    FakeTable::set_acquire_outcome(Capability::Overlay, AcquireOutcome::Granted);

    let world_draw = link.acquire(Capability::WorldDraw);
    let camera = link.acquire(Capability::Camera);
    let overlay = link.acquire(Capability::Overlay);
    let hud = link.acquire(Capability::Hud);

    assert_eq!(world_draw, Ok(AcquireOutcome::Denied { holder: ModIndex(2) }));
    assert_eq!(camera, Ok(AcquireOutcome::Preempted));
    assert_eq!(overlay, Ok(AcquireOutcome::Granted));
    assert_eq!(hud, Ok(AcquireOutcome::Granted));
    assert_eq!(
        FakeTable::with(|state| state.acquired.clone()),
        vec![Capability::WorldDraw, Capability::Camera, Capability::Overlay, Capability::Hud]
    );
}

#[test]
fn a_keybind_queued_in_each_phase_reaches_the_link_in_order() {
    let mut link = FakeTable::link_without_frame();
    let sent = [
        ModEvent::Keybind { action: 3, phase: KeybindPhase::Pressed },
        ModEvent::Keybind { action: 3, phase: KeybindPhase::Repeat },
        ModEvent::Keybind { action: 3, phase: KeybindPhase::Released },
    ];
    sent.iter().for_each(|event| FakeTable::push_event(*event));

    let received: Vec<ModEvent> = link.events().collect();

    assert_eq!(received, sent);
}

#[test]
fn lease_events_for_world_draw_reach_the_link_in_order() {
    let mut link = FakeTable::link_without_frame();
    let sent = [
        ModEvent::Granted(Capability::WorldDraw),
        ModEvent::Suspended { capability: Capability::WorldDraw, by: ModIndex(5) },
        ModEvent::Resumed(Capability::WorldDraw),
        ModEvent::Revoked { capability: Capability::WorldDraw, by: ModIndex(6) },
        ModEvent::Disabled,
    ];
    sent.iter().for_each(|event| FakeTable::push_event(*event));

    let received: Vec<ModEvent> = link.events().collect();

    assert_eq!(received, sent);
}

#[test]
fn release_and_set_setting_are_recorded_with_their_arguments() {
    let link = FakeTable::link_without_frame();

    link.release(Capability::WorldDraw);
    let set = link.set_setting("overlay.radius", &hcl::Value::from(24));

    assert_eq!(set, Ok(()));
    assert_eq!(FakeTable::with(|state| state.released.clone()), vec![Capability::WorldDraw]);
    assert_eq!(
        FakeTable::with(|state| state.settings.clone()),
        vec![SettingCall { path: "overlay.radius".to_owned(), expression: "24".to_owned() }]
    );
}

#[test]
fn request_screenshot_records_the_path_and_answers_the_sequence_or_status_set() {
    let link = FakeTable::link_without_frame();
    FakeTable::set_screenshot_answer(AmmStatus::Ok, 41);

    let first = link.request_screenshot(Path::new("C:\\shots\\a.png"));
    FakeTable::set_screenshot_answer(AmmStatus::Busy, 0);
    let second = link.request_screenshot(Path::new("C:\\shots\\b.png"));

    assert_eq!(first, Ok(41));
    assert_eq!(second, Err(LinkError::Status(AmmStatus::Busy as i32)));
    assert_eq!(
        FakeTable::with(|state| state.screenshot_requests.clone()),
        vec!["C:\\shots\\a.png".to_owned(), "C:\\shots\\b.png".to_owned()]
    );
}

#[test]
fn request_screenshot_with_passes_the_layer_choice() {
    let link = FakeTable::link_without_frame();
    FakeTable::set_screenshot_answer(AmmStatus::Ok, 9);

    let with_layers = link.request_screenshot_with(Path::new("C:\\shots\\a.png"), ScreenshotLayers::WithModLayers);
    let game_only = link.request_screenshot_with(Path::new("C:\\shots\\b.png"), ScreenshotLayers::GameOnly);
    let plain = link.request_screenshot(Path::new("C:\\shots\\c.png"));

    assert_eq!((with_layers, game_only, plain), (Ok(9), Ok(9), Ok(9)));
    assert_eq!(FakeTable::with(|state| state.screenshot_layers.clone()), vec![1, 0]);
    assert_eq!(
        FakeTable::with(|state| state.screenshot_requests.clone()),
        vec!["C:\\shots\\a.png".to_owned(), "C:\\shots\\b.png".to_owned(), "C:\\shots\\c.png".to_owned()]
    );
}

#[test]
fn both_screenshot_events_reach_the_link_in_order() {
    let mut link = FakeTable::link_without_frame();
    let sent =
        [ModEvent::ScreenshotTaken { sequence: 3, ok: true }, ModEvent::ScreenshotWritten { sequence: 3, ok: false }];
    sent.iter().for_each(|event| FakeTable::push_event(*event));

    let received: Vec<ModEvent> = link.events().collect();

    assert_eq!(received, sent);
}

#[test]
fn held_by_other_answers_what_is_set_per_capability_and_false_for_the_rest() {
    let link = FakeTable::link_without_frame();
    FakeTable::set_held_by_other(Capability::Camera, true);
    FakeTable::set_held_by_other(Capability::Input, false);

    let answers = [Capability::Camera, Capability::Input, Capability::Overlay].map(|cap| link.held_by_other(cap));

    assert_eq!(answers, [true, false, false]);
}

#[test]
fn texture_budget_answers_the_grant_set() {
    let link = FakeTable::link_without_frame();
    let grant = TextureGrant { bytes: 96 * 1024 * 1024, count: 12 };
    FakeTable::set_texture_grant(grant);

    assert_eq!(link.texture_budget(), Ok(grant));
}

#[test]
fn register_menu_entry_passes_label_mark_and_screens_and_returns_the_id() {
    let link = FakeTable::link_without_frame();
    FakeTable::set_menu_entry_id(4);

    let id = link.register_menu_entry("Dejavu", "DV", MenuScreens::START);

    assert_eq!(id, Ok(Some(MenuEntryId(4))));
    assert_eq!(FakeTable::with(|state| state.menu_entries.clone()), [("Dejavu".to_owned(), "DV".to_owned(), 1)]);
}

#[test]
fn client_reports_missing_slot_as_none() {
    let api = AmmApi { register_menu_entry: None, ..FakeTable::api() };
    let link = FakeTable::link_without_frame_on(api);

    assert_eq!(link.register_menu_entry("Dejavu", "DV", MenuScreens::START), Ok(None));
}

#[test]
fn a_menu_entry_click_reaches_the_link() {
    let mut link = FakeTable::link_without_frame();
    FakeTable::push_event(ModEvent::MenuEntryClicked { entry: 1 });

    let events: Vec<ModEvent> = link.events().collect();

    assert_eq!(events, [ModEvent::MenuEntryClicked { entry: 1 }]);
}
