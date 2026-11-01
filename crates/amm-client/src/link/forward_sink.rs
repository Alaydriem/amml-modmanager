use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use amm_abi::types::{AmmHandle, AmmLogLevel};
use curia::{Level, LogEvent, Sink};

use super::ApiTable;

/// A Curia sink that sends each event up to the manager through the table's `log`, which never blocks.
/// Clones share one switch: after `detach` no clone forwards again, because the handle is no longer the mod's.
#[derive(Debug, Clone)]
pub struct ForwardSink {
    table: ApiTable,
    handle: AmmHandle,
    level: Level,
    live: Arc<AtomicBool>,
}

impl ForwardSink {
    pub fn new(table: ApiTable, handle: AmmHandle, level: Level) -> Self {
        Self { table, handle, level, live: Arc::new(AtomicBool::new(true)) }
    }

    /// Stops this sink and every clone of it from forwarding.
    pub fn detach(&self) {
        self.live.store(false, Ordering::SeqCst);
    }

    fn code(level: Level) -> u8 {
        let level = match level {
            Level::Error => AmmLogLevel::Error,
            Level::Warn => AmmLogLevel::Warn,
            Level::Info => AmmLogLevel::Info,
            Level::Debug => AmmLogLevel::Debug,
            Level::Trace => AmmLogLevel::Trace,
        };
        level as u8
    }
}

impl Sink for ForwardSink {
    fn level(&self) -> Level {
        self.level
    }

    fn emit(&self, event: &LogEvent) {
        if !self.live.load(Ordering::SeqCst) {
            return;
        }
        let Some(log) = self.table.api().log else { return };
        // A map of JSON values always serialises; an empty document stands in should it ever fail.
        let fields = serde_json::to_string(&event.fields).unwrap_or_default();
        let file = event.file.as_deref().unwrap_or("");
        // SAFETY: every text pointer and length comes from a live &str or String borrowed for the call, and
        // the manager copies what it keeps; the handle is the mod's until detach, checked above
        unsafe {
            log(
                self.handle,
                Self::code(event.level),
                event.target.as_ptr(),
                event.target.len(),
                event.message.as_ptr(),
                event.message.len(),
                fields.as_ptr(),
                fields.len(),
                file.as_ptr(),
                file.len(),
                event.line.unwrap_or(0),
            );
        }
    }
}
