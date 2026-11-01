use amm_core::structs::mesh::{SamplerKind, TextureDelta};
use amm_ffi::services::TextureBudget;

const BUDGET: u64 = 1000;
const COUNT: usize = 8;
const MOD: u8 = 3;
const OTHER: u8 = 4;
const MANAGER: u8 = 0xFE;

fn full(id: u64, side: u32) -> TextureDelta {
    TextureDelta {
        id,
        sampler: SamplerKind::Linear,
        full: true,
        x: 0,
        y: 0,
        width: side,
        height: side,
        byte_offset: 0,
        byte_len: side * side * 4,
    }
}

fn partial(id: u64) -> TextureDelta {
    TextureDelta { full: false, width: 2, height: 2, byte_len: 16, ..full(id, 2) }
}

#[test]
fn delta_past_the_budget_is_refused_and_freeing_makes_room() {
    let mut budget = TextureBudget::new(BUDGET, COUNT);

    let first = budget.admit(MOD, &full(1, 10));
    let second = budget.admit(MOD, &full(2, 10));
    let over = budget.admit(MOD, &full(3, 10));
    let update_of_refused = budget.admit(MOD, &partial(3));
    let update_of_admitted = budget.admit(MOD, &partial(1));
    let used_when_full = budget.used(MOD);
    budget.free(MOD, 1);
    let after_free = budget.admit(MOD, &full(3, 10));
    let replace = budget.admit(MOD, &full(2, 10));

    assert!(first && second && update_of_admitted && after_free && replace);
    assert!(!over && !update_of_refused);
    assert_eq!(used_when_full, 800);
    assert_eq!(budget.used(MOD), 800);
}

#[test]
fn each_owner_has_its_own_budget_and_free_owner_empties_it() {
    let mut budget = TextureBudget::new(BUDGET, COUNT);
    budget.admit(MOD, &full(1, 15));
    budget.admit(MANAGER, &full(1, 15));

    let other = budget.admit(OTHER, &full(1, 15));
    budget.free_owner(MOD);

    assert!(other);
    assert_eq!(budget.used(MOD), 0);
    assert_eq!(budget.used(OTHER), 900);
    assert_eq!(budget.used(MANAGER), 900);
    assert!(budget.admit(MOD, &full(9, 15)));
}

#[test]
fn an_owner_past_the_slot_range_is_refused() {
    let mut budget = TextureBudget::new(BUDGET, COUNT);

    assert!(!budget.admit(40, &full(1, 2)));
    assert_eq!(budget.used(40), 0);
}

#[test]
fn a_new_texture_past_the_count_cap_is_refused_and_a_replace_is_not() {
    let mut budget = TextureBudget::new(BUDGET, 2);

    let first = budget.admit(MOD, &full(1, 1));
    let second = budget.admit(MOD, &full(2, 1));
    let third = budget.admit(MOD, &full(3, 1));
    let replace = budget.admit(MOD, &full(2, 2));
    let other_owner = budget.admit(OTHER, &full(3, 1));
    budget.free(MOD, 1);
    let after_free = budget.admit(MOD, &full(3, 1));

    assert!(first && second && replace && other_owner && after_free);
    assert!(!third);
}

#[test]
fn an_owner_limit_replaces_the_default_until_it_is_cleared() {
    let mut budget = TextureBudget::new(BUDGET, 1);
    budget.set_limit(MOD, 2 * BUDGET, 3);

    let granted = [1, 2, 3].map(|id| budget.admit(MOD, &full(id, 10)));
    let past_count = budget.admit(MOD, &full(4, 1));
    let other_default = [1, 2].map(|id| budget.admit(OTHER, &full(id, 1)));
    budget.free_owner(MOD);
    budget.clear_limit(MOD);
    let after_clear = [1, 2].map(|id| budget.admit(MOD, &full(id, 1)));

    assert_eq!(granted, [true, true, true]);
    assert!(!past_count);
    assert_eq!(other_default, [true, false]);
    assert_eq!(after_clear, [true, false]);
}
