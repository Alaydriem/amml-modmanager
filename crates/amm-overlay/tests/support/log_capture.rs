use std::sync::Mutex;

use curia::{Dispatcher, InstallError, Level, LogEvent, Logger, Sink};

static EVENTS: Mutex<Vec<LogEvent>> = Mutex::new(Vec::new());

/// A Curia sink that keeps every event in memory. The logger is global, so install it once per test process.
pub struct LogCapture;

impl LogCapture {
    pub fn install() {
        match Logger::install(Box::new(Dispatcher::new(vec![LogCapture]))) {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
    }

    /// How many events with this message came so far.
    pub fn count(message: &str) -> usize {
        EVENTS.lock().unwrap().iter().filter(|event| event.message == message).count()
    }

    /// The `(width, height)` of every `overlay bound` line so far, oldest first.
    pub fn bound_sizes() -> Vec<(u64, u64)> {
        let events = EVENTS.lock().unwrap();
        events
            .iter()
            .filter(|event| event.message == "overlay bound")
            .map(|event| {
                let field = |key: &str| event.fields.get(key).and_then(|value| value.as_u64()).unwrap();
                (field("width"), field("height"))
            })
            .collect()
    }
}

impl Sink for LogCapture {
    fn level(&self) -> Level {
        Level::Info
    }

    fn emit(&self, event: &LogEvent) {
        EVENTS.lock().unwrap().push(event.clone());
    }
}
