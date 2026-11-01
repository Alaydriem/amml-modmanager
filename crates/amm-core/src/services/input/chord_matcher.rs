use crate::structs::descriptor::Chord;
use crate::structs::registry::ModIndex;

/// Finds the keybinds bound to a chord. The entries are kept sorted by chord in one buffer that is
/// reused by every `rebuild`. The bound is `max_entries`; entries past it are dropped and counted.
pub struct ChordMatcher {
    entries: Vec<(Chord, ModIndex, u16)>,
    max_entries: usize,
    dropped: usize,
}

impl ChordMatcher {
    pub fn new(max_entries: usize) -> Self {
        Self { entries: Vec::with_capacity(max_entries), max_entries, dropped: 0 }
    }

    /// Replaces the entries with `chords`, each the mod, the keybind's index in its descriptor and the chord.
    pub fn rebuild(&mut self, chords: &[(ModIndex, u16, Chord)]) {
        self.entries.clear();
        let kept = chords.len().min(self.max_entries);
        self.entries.extend(chords[..kept].iter().map(|(index, action, chord)| (*chord, *index, *action)));
        self.entries.sort_unstable();
        self.dropped = chords.len() - kept;
        if self.dropped > 0 {
            curia::warn!("keybind table is full, keybinds dropped", { dropped: self.dropped, bound: self.max_entries });
        }
    }

    /// The keybinds bound to `chord`, in mod slot order then keybind order.
    pub fn find(&self, chord: Chord) -> impl Iterator<Item = (ModIndex, u16)> + '_ {
        let start = self.entries.partition_point(|entry| entry.0 < chord);
        self.entries[start..].iter().take_while(move |entry| entry.0 == chord).map(|entry| (entry.1, entry.2))
    }

    /// The number of entries the last `rebuild` dropped for the bound.
    pub fn dropped(&self) -> usize {
        self.dropped
    }
}
