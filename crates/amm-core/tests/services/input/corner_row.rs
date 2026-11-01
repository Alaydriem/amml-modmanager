use amm_core::services::input::CornerRow;
use amm_core::structs::menu::{CornerSlot, CornerTarget, MenuEntryId};
use amm_core::structs::registry::ModIndex;

const DEJAVU: CornerTarget = CornerTarget::Entry { owner: ModIndex(0), id: MenuEntryId(1) };
const MAP: CornerTarget = CornerTarget::Entry { owner: ModIndex(1), id: MenuEntryId(1) };

fn layout(entries: &[(CornerTarget, f32)], width: f32, height: f32) -> Vec<CornerSlot> {
    let mut out = Vec::new();
    CornerRow::layout(width, height, entries, &mut out);
    out
}

#[test]
fn corner_row_puts_mods_rightmost_and_entries_to_its_left() {
    let slots = layout(&[(DEJAVU, 110.0)], 1920.0, 1080.0);

    assert_eq!(slots.len(), 2);
    assert_eq!(slots[0].target, CornerTarget::Mods);
    assert_eq!(slots[0].rect, [1851.0, 987.0, 1896.0, 1032.0]);
    assert_eq!(slots[1].target, DEJAVU);
    assert_eq!(slots[1].rect, [1733.0, 987.0, 1843.0, 1032.0]);
}

#[test]
fn entries_run_right_to_left_in_registration_order() {
    let slots = layout(&[(DEJAVU, 110.0), (MAP, 80.0)], 1920.0, 1080.0);

    let targets: Vec<CornerTarget> = slots.iter().map(|slot| slot.target).collect();
    assert_eq!(targets, [CornerTarget::Mods, DEJAVU, MAP]);
    assert_eq!(slots[2].rect[2], slots[1].rect[0] - CornerRow::GAP);
}

#[test]
fn mods_alone_is_one_square_slot() {
    let slots = layout(&[], 1280.0, 720.0);

    assert_eq!(slots.len(), 1);
    assert_eq!(slots[0].rect, [1211.0, 627.0, 1256.0, 672.0]);
}

#[test]
fn bounds_cover_every_slot() {
    let slots = layout(&[(DEJAVU, 110.0), (MAP, 80.0)], 1920.0, 1080.0);

    let bounds = CornerRow::bounds(&slots);

    assert_eq!(bounds, Some([slots[2].rect[0], 987.0, 1896.0, 1032.0]));
    assert_eq!(CornerRow::bounds(&[]), None);
}
