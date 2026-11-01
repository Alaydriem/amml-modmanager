use amm_abi::types::{AmmAcquireOutcome, AmmCapability, AmmCursorShape, AmmEventKind, AmmUiEventKind};
use amm_core::structs::frame::FrameInputData;
use amm_ffi::host::{amm_host_char, amm_host_pointer};
use amm_ffi::types::AmmHostPointerKind;

use crate::support::test_client::SeenEvent;
use crate::support::{Descriptors, TestClient, TestHost, TestMesh};

const KEY_P: u32 = 0x50;
const KEY_Q: u32 = 0x51;

#[test]
fn key_route_consumes_a_blocked_keybind_and_queues_a_toast() {
    let host = TestHost::new();
    let freecam = TestClient::register(&host, Descriptors::FREECAM, TestMesh::triangle());
    let dejavu = TestClient::register(&host, Descriptors::DEJAVU, TestMesh::triangle());
    assert_eq!(dejavu.acquire(AmmCapability::Camera).outcome, AmmAcquireOutcome::Granted);
    freecam.poll();
    assert_eq!(host.frame(0).layer_count, 0);

    let pressed = host.key(KEY_P, true, TestHost::CTRL, 10);
    let released = host.key(KEY_P, false, TestHost::CTRL, 11);
    let frame = host.frame(12);

    assert!(pressed.consume);
    assert!(released.consume);
    assert!(freecam.poll().iter().all(|event| event.kind != AmmEventKind::Keybind));
    assert_eq!(TestHost::owners(&frame), vec![TestHost::MANAGER]);
}

#[test]
fn a_key_nobody_wants_reaches_the_game() {
    let host = TestHost::new();
    let _client = TestClient::register(&host, &Descriptors::overlay("quiet"), TestMesh::triangle());

    let route = host.key(KEY_Q, true, 0, 0);
    let char_route = amm_host_char(u32::from('q'));
    let pointer = amm_host_pointer(AmmHostPointerKind::Move as u8, 10.0, 10.0, 0, false, 0.0);

    assert!(!route.consume);
    assert!(!char_route.consume);
    assert!(!pointer.consume);
}

#[test]
fn the_open_panel_takes_every_key_and_pointer_event() {
    let host = TestHost::new();
    host.key(0x79, true, TestHost::CTRL | TestHost::SHIFT, 0);
    host.key(0x79, false, TestHost::CTRL | TestHost::SHIFT, 0);

    let route = host.key(KEY_Q, true, 0, 1);
    let pointer = amm_host_pointer(AmmHostPointerKind::Button as u8, 10.0, 10.0, 0, true, 0.0);
    let bad_kind = amm_host_pointer(99, 10.0, 10.0, 0, true, 0.0);

    assert!(route.consume);
    assert!(pointer.consume);
    assert!(!bad_kind.consume);
}

#[test]
fn a_pointer_move_reaches_the_frame_input_of_a_drawn_client() {
    let host = TestHost::new();
    let client = TestClient::register(&host, &Descriptors::overlay("hover"), TestMesh::triangle());
    client.acquire(AmmCapability::Overlay);

    amm_host_pointer(AmmHostPointerKind::Move as u8, 10.0, 10.0, 0, false, 0.0);
    amm_host_pointer(AmmHostPointerKind::Move as u8, 20.0, 20.0, 0, false, 0.0);
    host.frame(0);
    host.frame(1);

    assert_eq!(client.state().events_seen.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn a_key_held_by_an_input_holder_before_the_panel_opens_is_released_in_its_egui() {
    let host = TestHost::new();
    let holder = TestClient::register(
        &host,
        &Descriptors::with_claims("holder", &[("Overlay", "Shared"), ("Input", "Shared")], ""),
        TestMesh::triangle(),
    );
    holder.acquire(AmmCapability::Overlay);
    assert_eq!(holder.acquire(AmmCapability::Input).outcome, AmmAcquireOutcome::Granted);
    host.key(KEY_Q, true, 0, 0);
    host.frame(1);
    let seen_down = holder.state().events_seen.load(std::sync::atomic::Ordering::SeqCst);
    host.key(0x79, true, TestHost::CTRL | TestHost::SHIFT, 2);
    host.key(0x79, false, TestHost::CTRL | TestHost::SHIFT, 3);

    let released = host.key(KEY_Q, false, 0, 4);
    host.frame(5);

    assert_eq!(seen_down, 1);
    assert!(released.consume);
    assert_eq!(holder.state().events_seen.load(std::sync::atomic::Ordering::SeqCst), 2);
}

fn centre([min_x, min_y, max_x, max_y]: [f32; 4]) -> (f32, f32) {
    ((min_x + max_x) / 2.0, (min_y + max_y) / 2.0)
}

fn pointer(kind: AmmHostPointerKind, (x, y): (f32, f32), down: bool) -> bool {
    amm_host_pointer(kind as u8, x, y, 0, down, 1.0).consume
}

#[test]
fn clicking_the_menu_button_opens_the_panel_and_is_not_passed_to_the_game() {
    let host = TestHost::new();
    host.frame_with(0, TestHost::START);
    let at = centre(TestHost::menu_button_rect().expect("the button shows on a menu"));

    let moved = pointer(AmmHostPointerKind::Move, at, false);
    host.frame_with(1, TestHost::START);
    let pressed = pointer(AmmHostPointerKind::Button, at, true);
    host.frame_with(2, TestHost::START);
    let released = pointer(AmmHostPointerKind::Button, at, false);
    host.frame_with(3, TestHost::START);

    assert!(!moved, "a move over the button still reaches the game");
    assert!(pressed, "the press on the button is kept from the game");
    assert!(released, "the release that ends the click is kept from the game");
    assert!(TestHost::panel_open(), "the click opens the panel");
    assert_eq!(TestHost::menu_button_rect(), None, "the open panel hides the button");
}

#[test]
fn pointer_events_outside_the_menu_button_reach_the_game() {
    let host = TestHost::new();
    host.frame_with(0, TestHost::START);
    let on_button = centre(TestHost::menu_button_rect().expect("the button shows on a menu"));
    let elsewhere = (10.0, 10.0);

    let outside = [
        pointer(AmmHostPointerKind::Move, elsewhere, false),
        pointer(AmmHostPointerKind::Button, elsewhere, true),
        pointer(AmmHostPointerKind::Button, elsewhere, false),
        pointer(AmmHostPointerKind::Wheel, elsewhere, false),
    ];
    host.frame_with(1, TestHost::START);
    host.frame_with(2, TestHost::PLAY);
    let in_play = [
        pointer(AmmHostPointerKind::Move, on_button, false),
        pointer(AmmHostPointerKind::Button, on_button, true),
        pointer(AmmHostPointerKind::Button, on_button, false),
    ];
    host.frame_with(3, TestHost::PLAY);

    assert_eq!(outside, [false; 4]);
    assert_eq!(in_play, [false; 3], "during play the button's corner belongs to the game");
    assert!(!TestHost::panel_open());
}

#[test]
fn the_pointer_goes_to_the_overlay_layer_before_the_hud_layer_of_the_same_mod() {
    let host = TestHost::new();
    let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
    let both = TestClient::register(&host, &Descriptors::with_claims("both", &claims, ""), TestMesh::triangle());
    both.acquire(AmmCapability::Hud);
    both.acquire(AmmCapability::Overlay);
    both.reply_for_layer(0, (true, false, AmmCursorShape::Hand as u8, [0.0, 0.0, 200.0, 200.0]));
    both.reply_for_layer(1, (true, false, AmmCursorShape::Text as u8, [0.0, 0.0, 100.0, 100.0]));
    host.frame(0);

    pointer(AmmHostPointerKind::Move, (50.0, 50.0), false);
    let over_both = host.frame(1).cursor;
    pointer(AmmHostPointerKind::Move, (150.0, 150.0), false);
    let over_hud = host.frame(2).cursor;
    let pressed = pointer(AmmHostPointerKind::Button, (150.0, 150.0), true);
    pointer(AmmHostPointerKind::Button, (150.0, 150.0), false);

    assert_eq!(over_both, AmmCursorShape::Text);
    assert_eq!(over_hud, AmmCursorShape::Hand);
    assert!(pressed, "the Hud layer under the pointer still takes the click");
}

fn two_layer_client(host: &TestHost, id: &str) -> TestClient {
    let claims = [("Hud", "Shared"), ("Overlay", "Shared")];
    let client = TestClient::register(host, &Descriptors::with_claims(id, &claims, ""), TestMesh::triangle());
    client.acquire(AmmCapability::Hud);
    client.acquire(AmmCapability::Overlay);
    client
}

// The events each layer of `client` got in its last frame, Hud then Overlay.
fn last_frame_events(client: &TestClient) -> (Vec<SeenEvent>, Vec<SeenEvent>) {
    let calls = client.seen_calls();
    let last = &calls[calls.len() - 2..];
    assert_eq!((last[0].0, last[1].0), (1, 2));
    (last[0].1.clone(), last[1].1.clone())
}

#[test]
fn a_click_on_the_overlay_layer_does_not_reach_the_hud_layer_of_the_same_mod() {
    let host = TestHost::new();
    let both = two_layer_client(&host, "both");
    both.reply_for_layer(0, (true, false, 0, [0.0, 0.0, 200.0, 200.0]));
    both.reply_for_layer(1, (true, false, 0, [0.0, 0.0, 100.0, 100.0]));
    host.frame(0);

    pointer(AmmHostPointerKind::Move, (50.0, 50.0), false);
    let pressed = pointer(AmmHostPointerKind::Button, (50.0, 50.0), true);
    pointer(AmmHostPointerKind::Wheel, (50.0, 50.0), false);
    host.frame(1);
    let (hud, overlay) = last_frame_events(&both);

    let press = (AmmUiEventKind::PointerButton, true);
    assert!(pressed);
    assert!(overlay.contains(&press) && overlay.iter().any(|(kind, _)| *kind == AmmUiEventKind::Wheel));
    assert!(!hud.contains(&press), "the Hud layer got the press: {hud:?}");
    assert!(hud.iter().all(|(kind, _)| *kind != AmmUiEventKind::Wheel));
    assert!(hud.iter().any(|(kind, _)| *kind == AmmUiEventKind::PointerMove));
}

#[test]
fn text_goes_only_to_the_layer_that_wants_the_keyboard() {
    let host = TestHost::new();
    let both = two_layer_client(&host, "typist");
    both.reply_for_layer(0, (false, true, 0, [0.0; 4]));
    both.reply_for_layer(1, (false, false, 0, [0.0; 4]));
    host.frame(0);

    let typed = amm_host_char(0x61);
    host.frame(1);
    let (hud, overlay) = last_frame_events(&both);

    assert!(typed.consume);
    assert!(hud.iter().any(|(kind, _)| *kind == AmmUiEventKind::Char));
    assert!(overlay.iter().all(|(kind, _)| *kind != AmmUiEventKind::Char), "the Overlay layer got text: {overlay:?}");
}

#[test]
fn the_text_owner_gets_the_navigation_keys_without_an_input_lease() {
    let host = TestHost::new();
    let typist = two_layer_client(&host, "typist");
    typist.reply_for_layer(0, (false, true, 0, [0.0; 4]));
    host.frame(0);

    host.frame(1);

    assert_ne!(typist.input_flags() & FrameInputData::ALLOW_NAV_KEYS, 0);
}

#[test]
fn a_mod_with_no_field_and_no_input_lease_gets_no_navigation_keys() {
    let host = TestHost::new();
    let quiet = two_layer_client(&host, "quiet");
    host.frame(0);

    host.frame(1);

    assert_eq!(quiet.input_flags() & FrameInputData::ALLOW_NAV_KEYS, 0);
}
