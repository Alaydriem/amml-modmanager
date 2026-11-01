use std::sync::{Arc, Mutex, PoisonError};

use amml_i18n::localize::Localizer;

/// The localizer a mod's layers draw with, shared between the game thread, where the mod reads the manager's
/// language and sets it, and the render thread, where each layer pass reads it. English until the mod sets one.
#[derive(Clone)]
pub struct LocaleSlot {
    current: Arc<Mutex<Arc<Localizer>>>,
}

impl LocaleSlot {
    pub fn new() -> Self {
        Self { current: Arc::new(Mutex::new(Arc::new(Localizer::english()))) }
    }

    pub fn set(&self, localizer: Arc<Localizer>) {
        *self.current.lock().unwrap_or_else(PoisonError::into_inner) = localizer;
    }

    pub fn get(&self) -> Arc<Localizer> {
        Arc::clone(&self.current.lock().unwrap_or_else(PoisonError::into_inner))
    }
}

impl Default for LocaleSlot {
    fn default() -> Self {
        Self::new()
    }
}
