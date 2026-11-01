use amm_core::structs::frame::{UiEvent, UiEventKind};
use amm_ffi::services::UiEventBuffer;

fn key(vk: u32) -> UiEvent {
    UiEvent { kind: UiEventKind::Key, x: 0.0, y: 0.0, key: vk, modifiers: 0, down: true, ch: 0 }
}

fn pointer(x: f32) -> UiEvent {
    UiEvent { kind: UiEventKind::PointerMove, x, y: 0.0, key: 0, modifiers: 0, down: false, ch: 0 }
}

#[test]
fn a_push_past_capacity_drops_the_oldest_event_and_counts_it() {
    let mut buffer = UiEventBuffer::new(3);

    for vk in 1..=5 {
        buffer.push(key(vk));
    }

    let kept: Vec<u32> = buffer.as_slice().iter().map(|event| event.key).collect();
    assert_eq!(kept, vec![3, 4, 5]);
    assert_eq!(buffer.dropped(), 2);
}

#[test]
fn pointer_moves_in_a_row_merge_into_the_newest() {
    let mut buffer = UiEventBuffer::new(3);

    buffer.push(pointer(1.0));
    buffer.push(pointer(2.0));
    buffer.push(key(7));
    buffer.push(pointer(3.0));
    buffer.push(pointer(4.0));

    let kinds: Vec<(UiEventKind, f32)> = buffer.as_slice().iter().map(|event| (event.kind, event.x)).collect();
    assert_eq!(kinds, vec![(UiEventKind::PointerMove, 2.0), (UiEventKind::Key, 0.0), (UiEventKind::PointerMove, 4.0)]);
    assert_eq!(buffer.dropped(), 0);
}
