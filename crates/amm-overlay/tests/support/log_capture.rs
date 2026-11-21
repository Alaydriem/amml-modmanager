use std::sync::Mutex;

use curia::{Dispatcher, InstallError, Level, LogEvent, Logger, Sink};

static EVENTS: Mutex<Vec<LogEvent>> = Mutex::new(Vec::new());

/// A Curia sink that keeps every event in memory. The logger is global; nextest runs each test in its own process.
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
}

impl Sink for LogCapture {
    fn level(&self) -> Level {
        Level::Info
    }

    fn emit(&self, event: &LogEvent) {
        EVENTS.lock().unwrap().push(event.clone());
    }
}
