#[cfg(feature = "test-support")]
use std::sync::{Arc, Mutex, TryLockError};

use super::{ClipboardFault, WindowsClipboard};

/// Where a `ClipboardWorker` reads and writes text. Each call makes one attempt; the worker owns the retries.
#[derive(Clone)]
pub enum ClipboardBackend {
    /// The Windows clipboard.
    Windows,
    /// Text in memory, for tests. While a test holds the lock, the clipboard is busy, as when another program
    /// holds the Windows clipboard open.
    #[cfg(feature = "test-support")]
    Memory(Arc<Mutex<String>>),
    /// Text in memory, for tests. While a test holds the lock, each access waits for it, so the worker stays
    /// inside the request it took until the test lets go.
    #[cfg(feature = "test-support")]
    Blocking(Arc<Mutex<String>>),
}

impl ClipboardBackend {
    pub fn write(&self, text: &str) -> Result<(), ClipboardFault> {
        match self {
            Self::Windows => WindowsClipboard::write(text),
            #[cfg(feature = "test-support")]
            Self::Memory(memory) => {
                let mut held = Self::try_lock(memory)?;
                held.clear();
                held.push_str(text);
                Ok(())
            }
            #[cfg(feature = "test-support")]
            Self::Blocking(memory) => {
                let mut held = memory.lock().map_err(|_| ClipboardFault::Failed)?;
                held.clear();
                held.push_str(text);
                Ok(())
            }
        }
    }

    pub fn read(&self) -> Result<String, ClipboardFault> {
        match self {
            Self::Windows => WindowsClipboard::read(),
            #[cfg(feature = "test-support")]
            Self::Memory(memory) => Self::try_lock(memory).map(|held| held.clone()),
            #[cfg(feature = "test-support")]
            Self::Blocking(memory) => memory.lock().map(|held| held.clone()).map_err(|_| ClipboardFault::Failed),
        }
    }

    #[cfg(feature = "test-support")]
    fn try_lock(memory: &Mutex<String>) -> Result<std::sync::MutexGuard<'_, String>, ClipboardFault> {
        memory.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => ClipboardFault::Busy,
            TryLockError::Poisoned(_) => ClipboardFault::Failed,
        })
    }
}
