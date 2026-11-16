use crate::structs::input::MoveIntent;
use crate::structs::registry::{ModIndex, ModMask};

/// The one held move intent and the mod it belongs to. Game thread.
///
/// Bounds: one intent at a time. A mod that sets an intent replaces the intent of any other mod, and `set` names
/// that mod so it can be told. The all-zero intent clears.
pub struct FakeMove {
    owner: Option<ModIndex>,
    intent: MoveIntent,
}

impl FakeMove {
    pub fn new() -> Self {
        Self { owner: None, intent: MoveIntent::ZERO }
    }

    /// Holds `intent` for `index`, or clears when it is all zero. Returns the other mod whose intent it replaced.
    pub fn set(&mut self, index: ModIndex, intent: MoveIntent) -> Option<ModIndex> {
        let previous = self.owner.filter(|owner| *owner != index);
        if intent.is_zero() {
            if self.owner == Some(index) {
                self.reset();
            }
            return None;
        }
        self.owner = Some(index);
        self.intent = intent;
        previous
    }

    /// Clears the intent when `index` holds it. The answer holds `index` when it lost one.
    pub fn clear_mod(&mut self, index: ModIndex) -> ModMask {
        if self.owner == Some(index) { self.clear_all() } else { ModMask::default() }
    }

    /// Clears the intent of every mod except `keep`. The answer holds the mod that lost one.
    pub fn clear_except(&mut self, keep: ModIndex) -> ModMask {
        if self.owner == Some(keep) { ModMask::default() } else { self.clear_all() }
    }

    /// Clears the intent. The answer holds the mod that lost one.
    pub fn clear_all(&mut self) -> ModMask {
        let mut lost = ModMask::default();
        if let Some(owner) = self.owner {
            lost.insert(owner);
        }
        self.reset();
        lost
    }

    pub fn owner(&self) -> Option<ModIndex> {
        self.owner
    }

    /// The intent the game must see; all zero when no mod holds one.
    pub fn current(&self) -> MoveIntent {
        self.intent
    }

    fn reset(&mut self) {
        self.owner = None;
        self.intent = MoveIntent::ZERO;
    }
}

impl Default for FakeMove {
    fn default() -> Self {
        Self::new()
    }
}
