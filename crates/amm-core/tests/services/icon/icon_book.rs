use amm_core::services::icon::IconBook;
use amm_core::structs::icon::ModIcon;
use amm_core::structs::registry::ModIndex;

fn icon(side: u16) -> ModIcon {
    ModIcon { width: side, height: side, rgba: vec![255; usize::from(side) * usize::from(side) * 4] }
}

#[test]
fn set_replaces_and_raises_generation() {
    let mut book = IconBook::new(4);
    book.set(ModIndex(1), Some(icon(4)));
    let first = book.get(ModIndex(1)).map(|entry| entry.generation);
    book.set(ModIndex(1), Some(icon(4)));
    let second = book.get(ModIndex(1)).map(|entry| entry.generation);

    assert_eq!(book.len(), 1);
    assert!(second > first);
}

#[test]
fn clear_frees_the_slot() {
    let mut book = IconBook::new(4);
    book.set(ModIndex(2), Some(icon(8)));
    book.set(ModIndex(2), None);

    assert!(book.get(ModIndex(2)).is_none());
    assert_eq!(book.resident_bytes(), 0);
}

#[test]
fn bound_holds_past_capacity() {
    let mut book = IconBook::new(2);
    book.set(ModIndex(5), Some(icon(4)));
    book.set(ModIndex(0), Some(icon(4)));
    book.set(ModIndex(1), Some(icon(4)));
    book.forget(ModIndex(9));

    assert!(book.get(ModIndex(5)).is_none());
    assert!(book.len() <= 2);
}

#[test]
fn reused_slot_gets_a_new_generation() {
    let mut book = IconBook::new(2);
    book.set(ModIndex(0), Some(icon(4)));
    let first = book.get(ModIndex(0)).map(|entry| entry.generation);
    book.forget(ModIndex(0));
    book.set(ModIndex(0), Some(icon(4)));

    assert_ne!(book.get(ModIndex(0)).map(|entry| entry.generation), first);
}

#[test]
fn resident_bytes_counts_pixels() {
    let mut book = IconBook::new(2);
    book.set(ModIndex(0), Some(icon(256)));
    book.set(ModIndex(1), Some(icon(256)));

    assert_eq!(book.resident_bytes(), 2 * 262_144);
}
