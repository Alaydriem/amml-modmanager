use crate::errors::RegistryError;
use crate::structs::descriptor::Descriptor;
use crate::structs::registry::{ModIndex, ModMask, RegisteredMod};

/// Holds the registered mods in a fixed slot vector. The bound is `max_mods`, at most
/// `ModMask::CAPACITY`. A full registry refuses new mods and never evicts. A freed slot is reused.
pub struct ModRegistry {
    slots: Vec<Option<RegisteredMod>>,
}

impl ModRegistry {
    pub fn new(max_mods: usize) -> Self {
        Self { slots: vec![None; max_mods.min(ModMask::CAPACITY)] }
    }

    pub fn register(&mut self, descriptor: Descriptor, enabled: bool) -> Result<ModIndex, RegistryError> {
        let id = descriptor.id();
        if self.by_id(&id).is_some() {
            return Err(RegistryError::DuplicateId(id));
        }
        let Some(slot) = self.slots.iter().position(Option::is_none) else {
            return Err(RegistryError::TooManyMods);
        };
        let index = ModIndex(slot as u8);
        self.slots[slot] = Some(RegisteredMod { index, descriptor, enabled, failed: false });
        Ok(index)
    }

    pub fn unregister(&mut self, index: ModIndex) -> Option<RegisteredMod> {
        self.slots.get_mut(usize::from(index.0))?.take()
    }

    pub fn get(&self, index: ModIndex) -> Option<&RegisteredMod> {
        self.slots.get(usize::from(index.0))?.as_ref()
    }

    pub fn by_id(&self, id: &str) -> Option<ModIndex> {
        self.iter().find(|registered| registered.descriptor.id() == id).map(|registered| registered.index)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RegisteredMod> {
        self.slots.iter().flatten()
    }

    pub fn set_enabled(&mut self, index: ModIndex, enabled: bool) {
        if let Some(Some(registered)) = self.slots.get_mut(usize::from(index.0)) {
            registered.enabled = enabled;
        }
    }

    pub fn set_failed(&mut self, index: ModIndex, failed: bool) {
        if let Some(Some(registered)) = self.slots.get_mut(usize::from(index.0)) {
            registered.failed = failed;
        }
    }

    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
