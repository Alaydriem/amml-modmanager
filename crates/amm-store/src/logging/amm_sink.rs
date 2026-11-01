use curia::{Level, LogEvent, Sink};

use crate::logging::{GameConsoleSink, ModLogRouter};

/// The sinks the manager installs into Curia.
pub enum AmmSink {
    /// Routes the process-global events, which are the manager's own, to the manager slot.
    Router(ModLogRouter),
    Console(GameConsoleSink),
}

impl Sink for AmmSink {
    fn level(&self) -> Level {
        match self {
            Self::Router(router) => router.widest_level(),
            Self::Console(console) => console.level(),
        }
    }

    fn emit(&self, event: &LogEvent) {
        match self {
            Self::Router(router) => router.emit_for(ModLogRouter::MANAGER_SLOT, event),
            Self::Console(console) => console.emit(event),
        }
    }
}
