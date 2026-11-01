use amm_bench::fixtures::ChordTable;

#[test]
fn the_hit_chord_matches_and_the_miss_chord_does_not() {
    let table = ChordTable::new();

    assert!(table.count(table.hit()) >= 1);
    assert_eq!(table.count(table.miss()), 0);
}
