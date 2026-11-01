use amm_core::services::event::EventQueue;
use amm_core::structs::event::{KeybindPhase, ModEvent};

const CAPACITY: usize = 256;

fn keybind(action: u16) -> ModEvent {
    ModEvent::Keybind { action, phase: KeybindPhase::Pressed }
}

fn button(n: u8) -> ModEvent {
    ModEvent::PointerButton { button: n, down: true, x: 0.0, y: 0.0 }
}

fn key_up(vk: u32) -> ModEvent {
    ModEvent::Key { vk, down: false, repeat: false }
}

fn drain(queue: &mut EventQueue) -> Vec<ModEvent> {
    std::iter::from_fn(|| queue.pop()).collect()
}

#[test]
fn event_queue_past_capacity_merges_deltas_then_drops_oldest_keybind() {
    let mut queue = EventQueue::new(CAPACITY);
    queue.push(ModEvent::PointerDelta { dx: 1.0, dy: 2.0 });
    queue.push(ModEvent::PointerDelta { dx: 3.0, dy: 4.0 });
    for action in 0..(CAPACITY as u16 - 2) {
        queue.push(keybind(action));
    }
    assert_eq!(queue.len(), CAPACITY);

    queue.push(button(1));

    assert_eq!(queue.len(), CAPACITY);
    assert_eq!(queue.dropped(), 0);

    queue.push(button(2));

    assert_eq!(queue.len(), CAPACITY);
    assert_eq!(queue.dropped(), 1);
    let events = drain(&mut queue);
    assert_eq!(events[0], ModEvent::PointerDelta { dx: 4.0, dy: 6.0 });
    assert_eq!(events[1], keybind(1));
    assert_eq!(events[events.len() - 2], button(1));
    assert_eq!(events[events.len() - 1], button(2));
    assert!(!events.contains(&keybind(0)));
}

#[test]
fn full_queue_drops_keybind_then_wheel_then_key_repeat_before_the_new_event() {
    let mut queue = EventQueue::new(4);
    queue.push(ModEvent::Key { vk: 1, down: true, repeat: true });
    queue.push(ModEvent::Wheel { delta: 1.0 });
    queue.push(keybind(7));
    queue.push(button(0));

    queue.push(button(1));
    queue.push(button(2));
    queue.push(button(3));

    assert_eq!(queue.dropped(), 3);
    assert_eq!(drain(&mut queue), vec![button(0), button(1), button(2), button(3)]);

    for n in 0..4 {
        queue.push(button(n));
    }
    queue.push(button(9));

    assert_eq!(queue.dropped(), 4);
    assert_eq!(drain(&mut queue), vec![button(0), button(1), button(2), button(3)]);
}

#[test]
fn event_queue_never_drops_a_key_up() {
    let mut queue = EventQueue::new(CAPACITY);
    for n in 0..CAPACITY {
        queue.push(button((n % 200) as u8));
    }

    for vk in 0..10 {
        queue.push(key_up(vk));
    }

    assert_eq!(queue.len(), CAPACITY);
    assert_eq!(queue.dropped(), 10);
    let events = drain(&mut queue);
    let ups: Vec<ModEvent> = events.iter().copied().filter(|event| matches!(event, ModEvent::Key { .. })).collect();
    assert_eq!(ups, (0..10).map(key_up).collect::<Vec<_>>());
}

#[test]
fn keybind_releases_and_button_ups_are_kept_when_the_queue_is_full() {
    let released = ModEvent::Keybind { action: 1, phase: KeybindPhase::Released };
    let button_up = ModEvent::PointerButton { button: 0, down: false, x: 0.0, y: 0.0 };
    let mut queue = EventQueue::new(3);
    queue.push(keybind(0));
    queue.push(released);
    queue.push(button(0));

    queue.push(button_up);
    queue.push(released);
    queue.push(keybind(2));
    queue.push(button(1));

    assert_eq!(drain(&mut queue), vec![released, button_up, released]);
    assert_eq!(queue.dropped(), 4);
}

#[test]
fn a_full_queue_drops_pressed_and_repeat_keybinds_before_released() {
    let released = ModEvent::Keybind { action: 1, phase: KeybindPhase::Released };
    let repeat = ModEvent::Keybind { action: 2, phase: KeybindPhase::Repeat };
    let mut queue = EventQueue::new(4);
    queue.push(released);
    queue.push(keybind(3));
    queue.push(repeat);
    queue.push(released);

    queue.push(key_up(7));
    queue.push(key_up(8));

    assert_eq!(queue.dropped(), 2);
    assert_eq!(drain(&mut queue), vec![released, released, key_up(7), key_up(8)]);
}

#[test]
fn a_full_queue_never_drops_a_key_up_or_a_button_up() {
    let button_up = ModEvent::PointerButton { button: 0, down: false, x: 0.0, y: 0.0 };
    let mut queue = EventQueue::new(CAPACITY);
    for n in 0..CAPACITY / 2 {
        queue.push(ModEvent::Key { vk: n as u32, down: true, repeat: false });
        queue.push(ModEvent::PointerDelta { dx: 1.0, dy: 1.0 });
    }
    assert_eq!(queue.len(), CAPACITY);

    queue.push(key_up(5));
    queue.push(button_up);

    let events = drain(&mut queue);
    assert_eq!(events.len(), CAPACITY);
    assert!(events.contains(&key_up(5)));
    assert!(events.contains(&button_up));
}

#[test]
fn a_full_queue_drops_an_old_key_down_before_a_new_key_down_is_lost() {
    let down = |vk| ModEvent::Key { vk, down: true, repeat: false };
    let mut queue = EventQueue::new(3);
    queue.push(down(1));
    queue.push(down(2));
    queue.push(down(3));

    queue.push(down(4));

    assert_eq!(queue.dropped(), 1);
    assert_eq!(drain(&mut queue), vec![down(2), down(3), down(4)]);
}

#[test]
fn locale_event_survives_a_full_queue() {
    let mut queue = EventQueue::new(3);
    for n in 0..3 {
        queue.push(button(n));
    }

    queue.push(ModEvent::LocaleChanged);

    assert!(drain(&mut queue).contains(&ModEvent::LocaleChanged));
}
