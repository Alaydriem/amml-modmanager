use amm_core::services::input::ChordMatcher;
use amm_core::structs::descriptor::Chord;
use amm_core::structs::registry::ModIndex;

const KEY_M: u32 = 0x4D;

fn chord(ctrl: bool, shift: bool, alt: bool) -> Chord {
    Chord { key: KEY_M, ctrl, shift, alt, f3: false }
}

#[test]
fn chord_matcher_finds_ctrl_shift_m_and_not_ctrl_m() {
    let mut matcher = ChordMatcher::new(16);
    matcher.rebuild(&[
        (ModIndex(0), 0, chord(true, false, false)),
        (ModIndex(3), 2, chord(true, true, false)),
        (ModIndex(2), 1, chord(false, false, false)),
        (ModIndex(1), 3, chord(true, true, false)),
    ]);

    let found: Vec<(ModIndex, u16)> = matcher.find(chord(true, true, false)).collect();

    assert_eq!(found, vec![(ModIndex(1), 3), (ModIndex(3), 2)]);
    assert_eq!(matcher.find(chord(true, false, false)).collect::<Vec<_>>(), vec![(ModIndex(0), 0)]);
    assert_eq!(matcher.find(chord(true, false, true)).count(), 0);
    assert_eq!(matcher.find(chord(false, true, false)).count(), 0);
}

#[test]
fn chord_matcher_drops_entries_past_its_bound_and_counts_them() {
    let mut matcher = ChordMatcher::new(2);

    matcher.rebuild(&[
        (ModIndex(0), 0, chord(false, false, false)),
        (ModIndex(1), 0, chord(true, false, false)),
        (ModIndex(2), 0, chord(true, true, false)),
    ]);

    assert_eq!(matcher.dropped(), 1);
    assert_eq!(matcher.find(chord(true, true, false)).count(), 0);
    assert_eq!(matcher.find(chord(true, false, false)).count(), 1);

    matcher.rebuild(&[(ModIndex(2), 0, chord(true, true, false))]);

    assert_eq!(matcher.dropped(), 0);
    assert_eq!(matcher.find(chord(true, true, false)).collect::<Vec<_>>(), vec![(ModIndex(2), 0)]);
    assert_eq!(matcher.find(chord(false, false, false)).count(), 0);
}
