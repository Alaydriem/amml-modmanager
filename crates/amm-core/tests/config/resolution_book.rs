use amm_core::config::{ExclusiveResolution, PairKey, ResolutionBook, SuspensionResolution};
use amm_core::structs::descriptor::Capability;

#[test]
fn resolution_lookup_ignores_pair_order() {
    let book = ResolutionBook::new()
        .with_exclusive(
            PairKey::new(Capability::Camera, "freecam", "dejavu"),
            ExclusiveResolution::Prefer("freecam".into()),
        )
        .with_suspension(PairKey::new(Capability::Hud, "freecam", "dejavu"), SuspensionResolution::Coexist);

    let reversed = PairKey::new(Capability::Camera, "dejavu", "freecam");
    assert_eq!(book.exclusive_for(&reversed), Some(&ExclusiveResolution::Prefer("freecam".into())));
    let reversed = PairKey::new(Capability::Hud, "dejavu", "freecam");
    assert_eq!(book.suspension_for(&reversed), Some(&SuspensionResolution::Coexist));
}

#[test]
fn unreviewed_pair_returns_none() {
    let book = ResolutionBook::new()
        .with_exclusive(PairKey::new(Capability::Camera, "a", "b"), ExclusiveResolution::DenySecond);

    assert_eq!(book.exclusive_for(&PairKey::new(Capability::Camera, "a", "c")), None);
    assert_eq!(book.exclusive_for(&PairKey::new(Capability::Input, "a", "b")), None);
    assert_eq!(book.suspension_for(&PairKey::new(Capability::Camera, "a", "b")), None);
}

#[test]
fn with_exclusive_leaves_the_original_book_unchanged() {
    let original = ResolutionBook::new();
    let key = PairKey::new(Capability::Camera, "a", "b");

    let changed = original.with_exclusive(key.clone(), ExclusiveResolution::DenySecond);

    assert_eq!(original.exclusive_for(&key), None);
    assert_eq!(changed.exclusive_for(&key), Some(&ExclusiveResolution::DenySecond));
}

#[test]
fn a_book_is_empty_until_a_resolution_of_either_kind_is_stored() {
    let key = PairKey::new(Capability::Camera, "a", "b");

    let empty = ResolutionBook::new();
    let with_exclusive = empty.with_exclusive(key.clone(), ExclusiveResolution::DenySecond);
    let with_suspension = empty.with_suspension(key, SuspensionResolution::Coexist);

    assert!(empty.is_empty());
    assert!(!with_exclusive.is_empty());
    assert!(!with_suspension.is_empty());
}
