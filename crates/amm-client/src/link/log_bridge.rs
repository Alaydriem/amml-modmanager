use amml_api::Logger;
use amml_api::amml_abi::AmmlLogLevel;
use curia::{Dispatcher, InstallError, Level, LogEvent, Sink};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// A Curia sink that writes each event to the mod's log through the loader's `Logger`, with the fields as
/// `key=value` pairs (a text value without quotes, any other value as JSON text). The loader filters by the mod's
/// level, so the bridge passes every level.
#[derive(Clone, Copy)]
pub struct LogBridge {
    log: Logger,
}

impl LogBridge {
    pub fn new(log: Logger) -> Self {
        Self { log }
    }

    /// Installs Curia in this DLL with a bridge to `log`, and routes `log` and `tracing` events to Curia. Curia,
    /// the log capture or a tracing subscriber already installed in this DLL counts as installed and is kept.
    pub fn install(log: Logger) {
        match curia::Logger::install(Box::new(Dispatcher::new(vec![Self::new(log)]))) {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        match curia::TracingBridge::install_log_capture() {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        // The only failure is a global subscriber already set by this DLL, which counts as installed
        let _ = tracing_subscriber::registry().with(curia::TracingBridge::to_global()).try_init();
    }

    fn level(level: Level) -> AmmlLogLevel {
        match level {
            Level::Error => AmmlLogLevel::ERROR,
            Level::Warn => AmmlLogLevel::WARN,
            Level::Info => AmmlLogLevel::INFO,
            Level::Debug => AmmlLogLevel::DEBUG,
            Level::Trace => AmmlLogLevel::TRACE,
        }
    }
}

impl Sink for LogBridge {
    fn level(&self) -> Level {
        Level::Trace
    }

    fn emit(&self, event: &LogEvent) {
        let values: Vec<(&str, String)> = event
            .fields
            .as_map()
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str().map_or_else(|| value.to_string(), str::to_owned)))
            .collect();
        let fields: Vec<(&str, &str)> = values.iter().map(|(key, value)| (*key, value.as_str())).collect();
        self.log.log(Self::level(event.level), &event.message, &fields);
    }
}
