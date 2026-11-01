use amm_core::services::egui_bridge::RawInputBuilder;
use amm_core::structs::frame::{FrameInputData, UiEvent, UiEventKind};
use egui::{Event, RawInput};

fn event(kind: UiEventKind) -> UiEvent {
    UiEvent { kind, x: 0.0, y: 0.0, key: 0, modifiers: 0, down: false, ch: 0 }
}

fn frame(events: &[UiEvent], allowed: u8) -> FrameInputData<'_> {
    FrameInputData {
        width: 1000,
        height: 500,
        pixels_per_point: 2.0,
        time_secs: 1.5,
        events,
        allowed_layers: FrameInputData::LAYER_OVERLAY,
        input_flags: allowed,
    }
}

#[test]
fn pointer_events_are_converted_to_points() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [UiEvent { x: 100.0, y: 60.0, ..event(UiEventKind::PointerMove) }];

    builder.build(&frame(&events, FrameInputData::ALLOW_POINTER), &mut raw);

    assert_eq!(raw.events, vec![Event::PointerMoved(egui::pos2(50.0, 30.0))]);
    assert_eq!(raw.screen_rect.map(|r| r.size()), Some(egui::vec2(500.0, 250.0)));
}

#[test]
fn disallowed_pointer_events_are_dropped() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [event(UiEventKind::PointerMove)];

    builder.build(&frame(&events, FrameInputData::ALLOW_KEYBOARD), &mut raw);

    assert!(raw.events.is_empty());
}

#[test]
fn space_is_dropped_without_nav_keys_and_delivered_with_them() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [UiEvent { key: 0x20, down: true, ..event(UiEventKind::Key) }];

    builder.build(&frame(&events, FrameInputData::ALLOW_KEYBOARD), &mut raw);
    assert!(raw.events.is_empty());

    let allowed = FrameInputData::ALLOW_KEYBOARD | FrameInputData::ALLOW_NAV_KEYS;
    builder.build(&frame(&events, allowed), &mut raw);
    assert_eq!(raw.events.len(), 1);
}

#[test]
fn focus_loss_releases_held_buttons() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [UiEvent { down: true, ..event(UiEventKind::PointerButton) }, event(UiEventKind::FocusLost)];

    builder.build(&frame(&events, FrameInputData::ALLOW_POINTER), &mut raw);

    let released = raw.events.iter().any(|e| matches!(e, Event::PointerButton { pressed: false, .. }));
    assert!(released);
    assert_eq!(raw.events.last(), Some(&Event::PointerGone));
}

#[test]
fn ctrl_v_down_with_the_keyboard_allowed_records_a_paste_request() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [UiEvent { key: 0x56, modifiers: UiEvent::MOD_CTRL, down: true, ..event(UiEventKind::Key) }];

    builder.build(&frame(&events, FrameInputData::ALLOW_KEYBOARD), &mut raw);

    assert!(builder.take_paste_request());
    assert!(!builder.take_paste_request());
}

#[test]
fn ctrl_v_without_the_keyboard_records_none() {
    let mut builder = RawInputBuilder::new();
    let mut raw = RawInput::default();
    let events = [UiEvent { key: 0x56, modifiers: UiEvent::MOD_CTRL, down: true, ..event(UiEventKind::Key) }];

    builder.build(&frame(&events, FrameInputData::ALLOW_POINTER), &mut raw);

    assert!(!builder.take_paste_request());
}
