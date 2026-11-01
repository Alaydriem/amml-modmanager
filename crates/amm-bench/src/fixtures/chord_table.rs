use amm_core::services::input::ChordMatcher;
use amm_core::structs::descriptor::Chord;
use amm_core::structs::registry::ModIndex;

/// A chord matcher loaded with 64 binds across 8 mods.
pub struct ChordTable {
    matcher: ChordMatcher,
    hit: Chord,
    miss: Chord,
}

impl ChordTable {
    pub const BINDS: usize = 64;
    const MODS: u8 = 8;
    const FIRST_KEY: u32 = 0x41;

    pub fn new() -> Self {
        let mut chords = Vec::with_capacity(Self::BINDS);
        for n in 0..Self::BINDS {
            let key = Self::FIRST_KEY + (n % 26) as u32;
            let chord = Chord { key, ctrl: n % 3 == 0, shift: n % 3 == 1, alt: n >= 52, f3: false };
            chords.push((ModIndex((n % usize::from(Self::MODS)) as u8), (n / 8) as u16, chord));
        }
        let hit = chords[Self::BINDS / 2].2;
        let mut matcher = ChordMatcher::new(Self::BINDS * 2);
        matcher.rebuild(&chords);
        Self { matcher, hit, miss: Chord { key: 0x7F, ctrl: true, shift: true, alt: true, f3: false } }
    }

    pub fn hit(&self) -> Chord {
        self.hit
    }

    pub fn miss(&self) -> Chord {
        self.miss
    }

    /// How many binds `chord` matches.
    pub fn count(&self, chord: Chord) -> usize {
        self.matcher.find(chord).count()
    }
}

impl Default for ChordTable {
    fn default() -> Self {
        Self::new()
    }
}
