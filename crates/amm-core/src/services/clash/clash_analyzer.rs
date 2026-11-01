use crate::config::{PairKey, ResolutionBook};
use crate::services::registry::ModRegistry;
use crate::structs::clash::{BindRef, Clash, ClashKind, ClashReport};
use crate::structs::descriptor::{Capability, Chord, ClaimMode, KeyScope, KeybindDecl};
use crate::structs::registry::{ModIndex, ModMask, RegisteredMod};

/// Finds the clashes between the enabled mods. The report and the sort buffer are reused by every
/// analysis. The report holds at most one capability entry per capability and mod pair and at most
/// `MAX_KEY_CLASHES` key clashes.
pub struct ClashAnalyzer {
    report: ClashReport,
    sorted: Vec<(Chord, BindRef)>,
}

impl ClashAnalyzer {
    /// The most key clashes one analysis reports: one per bind of a full registry.
    pub const MAX_KEY_CLASHES: usize = ModMask::CAPACITY * 128;

    pub fn new() -> Self {
        Self { report: ClashReport::new(), sorted: Vec::new() }
    }

    pub fn analyze(
        &mut self,
        registry: &ModRegistry,
        book: &ResolutionBook,
        chords: &[(ModIndex, u16, Chord)],
    ) -> &ClashReport {
        self.report.clear();
        self.sorted.clear();
        self.find_capability_clashes(registry, book);
        self.find_key_clashes(registry, chords);
        &self.report
    }

    /// The report of the last analysis.
    pub fn report(&self) -> &ClashReport {
        &self.report
    }

    fn find_capability_clashes(&mut self, registry: &ModRegistry, book: &ResolutionBook) {
        for first in registry.iter().filter(|registered| registered.enabled) {
            let later = registry.iter().filter(|other| other.enabled && other.index > first.index);
            for second in later {
                for capability in Capability::ALL {
                    self.compare_claims(book, capability, first, second);
                }
            }
        }
    }

    fn compare_claims(
        &mut self,
        book: &ResolutionBook,
        capability: Capability,
        first: &RegisteredMod,
        second: &RegisteredMod,
    ) {
        let (Some(a), Some(b)) = (Self::claim_mode(first, capability), Self::claim_mode(second, capability)) else {
            return;
        };
        let key = PairKey::new(capability, &first.descriptor.id(), &second.descriptor.id());
        match (a, b) {
            (ClaimMode::Exclusive, ClaimMode::Exclusive) => self.report.conflicts.push(Clash {
                kind: ClashKind::Exclusive { capability, a: first.index, b: second.index },
                reviewed: book.exclusive_for(&key).is_some(),
            }),
            (ClaimMode::Exclusive, ClaimMode::Shared) => self.report.interactions.push(Clash {
                kind: ClashKind::Suspension { capability, exclusive: first.index, shared: second.index },
                reviewed: book.suspension_for(&key).is_some(),
            }),
            (ClaimMode::Shared, ClaimMode::Exclusive) => self.report.interactions.push(Clash {
                kind: ClashKind::Suspension { capability, exclusive: second.index, shared: first.index },
                reviewed: book.suspension_for(&key).is_some(),
            }),
            (ClaimMode::Shared, ClaimMode::Shared) => {}
        }
    }

    fn find_key_clashes(&mut self, registry: &ModRegistry, chords: &[(ModIndex, u16, Chord)]) {
        let live =
            chords.iter().filter(|(index, _, _)| registry.get(*index).is_some_and(|registered| registered.enabled));
        self.sorted.extend(live.map(|(index, action, chord)| (*chord, BindRef { mod_index: *index, action: *action })));
        self.sorted.sort_unstable();
        let limit = self.report.conflicts.len() + Self::MAX_KEY_CLASHES;
        let mut start = 0;
        while start < self.sorted.len() {
            let chord = self.sorted[start].0;
            let run = self.sorted[start..].iter().take_while(|(next, _)| *next == chord).count();
            self.compare_run(registry, start, run, limit);
            start += run;
        }
    }

    fn compare_run(&mut self, registry: &ModRegistry, start: usize, run: usize, limit: usize) {
        for i in start..start + run {
            for j in i + 1..start + run {
                let (chord, a) = self.sorted[i];
                let (_, b) = self.sorted[j];
                if Self::binds_clash(registry, a, b) {
                    if self.report.conflicts.len() >= limit {
                        self.report.truncated = true;
                        return;
                    }
                    self.report.conflicts.push(Clash { kind: ClashKind::Key { chord, a, b }, reviewed: false });
                }
            }
        }
    }

    fn binds_clash(registry: &ModRegistry, a: BindRef, b: BindRef) -> bool {
        let (Some(bind_a), Some(bind_b)) = (Self::decl(registry, a), Self::decl(registry, b)) else {
            return false;
        };
        match (bind_a.scope, bind_b.scope) {
            (KeyScope::Global, KeyScope::Global) => true,
            (KeyScope::Global, KeyScope::WhileHeld) => !Self::holds_input_exclusively(registry, b, bind_b),
            (KeyScope::WhileHeld, KeyScope::Global) => !Self::holds_input_exclusively(registry, a, bind_a),
            (KeyScope::WhileHeld, KeyScope::WhileHeld) => {
                !Self::leases_exclude_each_other(registry, (a, bind_a), (b, bind_b))
            }
        }
    }

    // A WhileHeld owner with an exclusive Input claim blocks Global binds while it holds the lease.
    fn holds_input_exclusively(registry: &ModRegistry, bind: BindRef, decl: &KeybindDecl) -> bool {
        decl.capability == Capability::Input
            && Self::mode_of(registry, bind.mod_index, Capability::Input) == Some(ClaimMode::Exclusive)
    }

    // Two leases cannot be held at once when they are on one capability and either side is exclusive.
    fn leases_exclude_each_other(
        registry: &ModRegistry,
        (a, decl_a): (BindRef, &KeybindDecl),
        (b, decl_b): (BindRef, &KeybindDecl),
    ) -> bool {
        a.mod_index != b.mod_index
            && decl_a.capability == decl_b.capability
            && (Self::mode_of(registry, a.mod_index, decl_a.capability) == Some(ClaimMode::Exclusive)
                || Self::mode_of(registry, b.mod_index, decl_b.capability) == Some(ClaimMode::Exclusive))
    }

    fn decl(registry: &ModRegistry, bind: BindRef) -> Option<&KeybindDecl> {
        registry.get(bind.mod_index)?.descriptor.keybinds.get(usize::from(bind.action))
    }

    fn mode_of(registry: &ModRegistry, index: ModIndex, capability: Capability) -> Option<ClaimMode> {
        Self::claim_mode(registry.get(index)?, capability)
    }

    fn claim_mode(registered: &RegisteredMod, capability: Capability) -> Option<ClaimMode> {
        registered.descriptor.claims.iter().find(|claim| claim.capability == capability).map(|claim| claim.mode)
    }
}

impl Default for ClashAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}
