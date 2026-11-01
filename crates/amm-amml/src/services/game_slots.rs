use std::sync::atomic::{AtomicU64, Ordering};

use amml_sdk::SdkError;

use crate::traits::HookHost;
use crate::types::GameSlot;

/// The vtable slot indexes of the `GameSlot` entries, read at enable.
pub struct GameSlots {
    // written with Release by enable before the hooks go in, read with Acquire by the game, input and window
    // threads that run those hooks
    values: [AtomicU64; GameSlot::COUNT],
}

impl GameSlots {
    const NONE: u64 = u64::MAX;

    pub const fn new() -> Self {
        Self { values: [const { AtomicU64::new(Self::NONE) }; GameSlot::COUNT] }
    }

    /// Reads every entry. A missing required entry fails with nothing kept; a missing optional entry logs one
    /// line naming its feature, and that feature stays off.
    pub fn resolve<H: HookHost>(&self, host: &H) -> Result<(), SdkError> {
        let mut found = [Self::NONE; GameSlot::COUNT];
        for slot in GameSlot::ALL {
            match (host.resolve(slot.entry()), slot.feature()) {
                (Ok(value), _) => found[slot as usize] = value,
                (Err(error), None) => return Err(error),
                (Err(error), Some(feature)) => {
                    host.warn(&format!("{} did not resolve ({error}); {feature} is off", slot.entry()));
                }
            }
        }
        for (value, found) in self.values.iter().zip(found) {
            value.store(found, Ordering::Release);
        }
        Ok(())
    }

    /// The value of `slot`, when it resolved.
    pub fn get(&self, slot: GameSlot) -> Option<usize> {
        match self.values[slot as usize].load(Ordering::Acquire) {
            Self::NONE => None,
            value => usize::try_from(value).ok(),
        }
    }

    /// All of `slots` resolved.
    pub fn all(&self, slots: &[GameSlot]) -> bool {
        slots.iter().all(|slot| self.get(*slot).is_some())
    }
}

impl Default for GameSlots {
    fn default() -> Self {
        Self::new()
    }
}
