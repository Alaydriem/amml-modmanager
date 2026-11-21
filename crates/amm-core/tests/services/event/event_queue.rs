use amm_core::services::event::EventQueue;
use amm_core::structs::event::{KeybindPhase, ModEvent};

const CAPACITY: usize = 256;

fn keybind(action: u16) -> ModEvent {
    ModEvent::Keybind { action, phase: KeybindPhase::Pressed }
}

fn released(action: u16) -> ModEvent {
    ModEvent::Keybind { action, phase: KeybindPhase::Released }
}

fn drain(queue: &mut EventQueue) -> Vec<ModEvent> {
    std::iter::from_fn(|| queue.pop()).collect()
}

#[test]
fn event_queue_past_capacity_drops_the_oldest_pressed_keybind() {
    let mut queue = EventQueue::new(CAPACITY);
    for action in 0..CAPACITY as u16 {
        queue.push(keybind(action));
    }
    assert_eq!(queue.len(), CAPACITY);

    queue.push(keybind(9000));

    assert_eq!(queue.len(), CAPACITY);
    assert_eq!(queue.dropped(), 1);
    let events = drain(&mut queue);
    assert_eq!(events[0], keybind(1));
    assert_eq!(events[events.len() - 1], keybind(9000));
    assert!(!events.contains(&keybind(0)));
}

#[test]
fn keybind_releases_are_kept_when_the_queue_is_full() {
    let mut queue = EventQueue::new(3);
    queue.push(keybind(0));
    queue.push(released(1));
    queue.push(keybind(2));

    queue.push(released(3));
    queue.push(released(4));
    queue.push(keybind(5));

    assert_eq!(drain(&mut queue), vec![released(1), released(3), released(4)]);
    assert_eq!(queue.dropped(), 3);
}

#[test]
fn a_full_queue_drops_pressed_and_repeat_keybinds_before_released() {
    let repeat = ModEvent::Keybind { action: 2, phase: KeybindPhase::Repeat };
    let mut queue = EventQueue::new(4);
    queue.push(released(1));
    queue.push(keybind(3));
    queue.push(repeat);
    queue.push(released(1));

    queue.push(ModEvent::SettingsChanged { paths: 1 });
    queue.push(ModEvent::WorldReloadApply);

    assert_eq!(queue.dropped(), 2);
    assert_eq!(
        drain(&mut queue),
        vec![released(1), released(1), ModEvent::SettingsChanged { paths: 1 }, ModEvent::WorldReloadApply]
    );
}

#[test]
fn a_queue_full_of_protected_events_drops_the_new_event() {
    let mut queue = EventQueue::new(2);
    queue.push(released(1));
    queue.push(ModEvent::Enabled);

    queue.push(ModEvent::LocaleChanged);
    queue.push(keybind(4));

    assert_eq!(queue.dropped(), 2);
    assert_eq!(drain(&mut queue), vec![released(1), ModEvent::Enabled]);
}

#[test]
fn locale_event_survives_a_full_queue_of_presses() {
    let mut queue = EventQueue::new(3);
    for action in 0..3 {
        queue.push(keybind(action));
    }

    queue.push(ModEvent::LocaleChanged);

    assert!(drain(&mut queue).contains(&ModEvent::LocaleChanged));
}
