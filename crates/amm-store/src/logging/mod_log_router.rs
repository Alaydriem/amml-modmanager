use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard};

use amm_core::errors::StoreError;
use amm_core::structs::registry::ModMask;
use curia::{DEFAULT_TIMEZONE_STRATEGY, FileOpenStrategy, FileSink, Level, LogEvent, RotationStrategy, Sink};

use crate::logging::{LineFormat, LogRoot, SessionStamp};

type Slot = RwLock<Option<FileSink>>;

/// Routes log events to one Curia file sink per slot. The slot count is fixed at construction, and each sink has
/// Curia's bounded queue. Clones share the same slots.
#[derive(Clone)]
pub struct ModLogRouter {
    root: LogRoot,
    stamp: SessionStamp,
    slots: Arc<[Slot]>,
    // opens that found a live sink in their slot and closed it first, which joins its writer thread
    replaced: Arc<AtomicU64>,
}

impl ModLogRouter {
    /// The slot of the manager's own log.
    pub const MANAGER_SLOT: u8 = ModMask::CAPACITY as u8;
    /// The size at which a session file rotates into an archive.
    pub const MAX_FILE_BYTES: u64 = 8 * 1024 * 1024;
    /// How many archives a session keeps beside its active file.
    pub const KEPT_ARCHIVES: usize = 4;

    pub fn new(root: LogRoot, stamp: SessionStamp, max_sinks: usize) -> ModLogRouter {
        let slots: Arc<[Slot]> = (0..max_sinks).map(|_| RwLock::new(None)).collect();
        Self { root, stamp, slots, replaced: Arc::new(AtomicU64::new(0)) }
    }

    /// Opens the session file `<stamp>.log` of the mod `@<namespace>/<name>` in `slot`. An open sink in that slot is closed first, which drains it.
    /// A slot past the fixed count is refused with `Unavailable`.
    pub fn open(&self, slot: u8, namespace: &str, name: &str, level: Level) -> Result<(), StoreError> {
        let Some(cell) = self.slots.get(usize::from(slot)) else {
            return Err(StoreError::Unavailable);
        };
        let previous = cell.write().unwrap_or_else(PoisonError::into_inner).take();
        if previous.is_some() {
            self.replaced.fetch_add(1, Ordering::Relaxed);
        }
        drop(previous);
        let dir = self.root.dir_for(namespace, name);
        let sink = FileSink::with_rotation(
            dir.clone(),
            self.stamp.as_str().to_string(),
            level,
            LineFormat::formatter(),
            Self::MAX_FILE_BYTES,
            RotationStrategy::KeepSome(Self::KEPT_ARCHIVES),
            DEFAULT_TIMEZONE_STRATEGY,
            FileOpenStrategy::Append,
        )
        .map_err(|_| StoreError::Io { path: dir })?;
        *cell.write().unwrap_or_else(PoisonError::into_inner) = Some(sink);
        Ok(())
    }

    /// Drops the sink in `slot`, which writes out its queue before this returns.
    pub fn close(&self, slot: u8) {
        if let Some(cell) = self.slots.get(usize::from(slot)) {
            let sink = cell.write().unwrap_or_else(PoisonError::into_inner).take();
            drop(sink);
        }
    }

    /// Gives the event to the sink in `slot` when one is open and admits the event's level. Never blocks on I/O.
    pub fn emit_for(&self, slot: u8, event: &LogEvent) {
        let Some(cell) = self.slots.get(usize::from(slot)) else {
            return;
        };
        if let Some(sink) = Self::read(cell).as_ref().filter(|sink| event.level <= sink.level()) {
            sink.emit(event);
        }
    }

    /// How many opens found a live sink in their slot and closed it first. Closing a sink joins its writer
    /// thread, so an open that replaces a sink must never run where a new thread cannot start.
    pub fn replaced(&self) -> u64 {
        self.replaced.load(Ordering::Relaxed)
    }

    /// Lines the sink in `slot` dropped because its queue was full.
    pub fn dropped(&self, slot: u8) -> u64 {
        self.slots.get(usize::from(slot)).and_then(|cell| Self::read(cell).as_ref().map(FileSink::dropped)).unwrap_or(0)
    }

    /// The most verbose level any open slot uses, or `Trace` when none is open.
    pub fn widest_level(&self) -> Level {
        let mut widest: Option<Level> = None;
        for cell in self.slots.iter() {
            if let Some(sink) = Self::read(cell).as_ref() {
                widest = Some(widest.map_or(sink.level(), |level| level.max(sink.level())));
            }
        }
        widest.unwrap_or(Level::Trace)
    }

    fn read(cell: &Slot) -> RwLockReadGuard<'_, Option<FileSink>> {
        cell.read().unwrap_or_else(PoisonError::into_inner)
    }
}
