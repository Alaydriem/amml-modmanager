use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use amm_abi::types::AmmLogLevel;
use amm_store::logging::game_console_sink::ConsoleReceiver;
use amm_store::logging::{AmmSink, GameConsoleSink, LogRetention, LogRoot, ModLogRouter, SessionStamp};
use curia::{Dispatcher, Fields, InstallError, Level, LogEvent, Logger, Sink, TracingBridge};
use time::OffsetDateTime;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The manager's logging for the life of the process: the session stamp, the router with one file sink per mod
/// slot plus the manager's, and the console channel the host mod drains. Curia can be installed once per process, so
/// the bridge is made once, by the first host, and kept by every later host. The router holds at most 33 sinks.
pub struct LogBridge {
    router: ModLogRouter,
    console: GameConsoleSink,
    console_rx: ConsoleReceiver,
}

impl LogBridge {
    /// The manager's own namespace and name, which name its log folder.
    pub const MANAGER_NAMESPACE: &'static str = "amml";
    pub const MANAGER_NAME: &'static str = "modmanager";
    /// The manager's own id, `@amml/modmanager`.
    pub const MANAGER_ID: &'static str = "@amml/modmanager";
    const FIELDS_UNPARSED: &'static str = "fields_unparsed";

    /// The bridge of this process, made on the first call with that call's game folder. No slot is open in a new
    /// bridge: each host opens the manager's slot once and closes it at shutdown.
    pub fn process(game_dir: &Path, sessions_kept: usize) -> &'static LogBridge {
        Self::cell().get_or_init(|| Self::new(game_dir, sessions_kept))
    }

    /// The bridge, once a host made it.
    pub fn current() -> Option<&'static LogBridge> {
        Self::cell().get()
    }

    /// Opens the session file of a mod in its slot.
    pub fn open(&self, slot: u8, namespace: &str, name: &str, level: Level) {
        if let Err(error) = self.router.open(slot, namespace, name, level) {
            curia::warn!("mod log file could not be opened", {
                namespace: namespace,
                name: name,
                error: error.to_string(),
            });
        }
    }

    /// Closes the session file in a slot, after its queue is written.
    pub fn close(&self, slot: u8) {
        self.router.close(slot);
    }

    /// How many opens replaced a log file that was still open in its slot.
    pub fn replaced_sinks(&self) -> u64 {
        self.router.replaced()
    }

    /// Rebuilds one log event from its parts and gives it to the file of `slot`; Warn and Error also go to the
    /// game console. Fields that are not a JSON object are kept as text in `fields_unparsed`.
    pub fn emit(&self, slot: u8, level: AmmLogLevel, parts: [String; 4], line: u32) {
        let [target, message, fields_json, file] = parts;
        let event = LogEvent {
            level: Self::level(level),
            target,
            message,
            fields: Self::fields(fields_json),
            timestamp: chrono::Utc::now(),
            file: Some(file).filter(|file| !file.is_empty()),
            line: Some(line).filter(|line| *line != 0),
        };
        self.router.emit_for(slot, &event);
        self.console.emit(&event);
    }

    /// Gives each queued console line to `sink`, at most the channel's capacity per call.
    pub fn drain_console(&self, mut sink: impl FnMut(AmmLogLevel, &str)) {
        for _ in 0..GameConsoleSink::CAPACITY {
            let Ok((level, line)) = self.console_rx.try_recv() else {
                return;
            };
            sink(Self::abi_level(level), &line);
        }
    }

    fn new(game_dir: &Path, sessions_kept: usize) -> LogBridge {
        let root = LogRoot::new(game_dir);
        let stamp = SessionStamp::new(OffsetDateTime::now_utc());
        Self::prune(&root, sessions_kept, &stamp);
        let router = ModLogRouter::new(root, stamp, usize::from(ModLogRouter::MANAGER_SLOT) + 1);
        let (tx, console_rx) = GameConsoleSink::channel();
        let bridge = Self { router, console: GameConsoleSink::new(tx.clone()), console_rx };
        let sinks = vec![AmmSink::Router(bridge.router.clone()), AmmSink::Console(GameConsoleSink::new(tx))];
        Self::install(Dispatcher::new(sinks));
        bridge
    }

    fn install(dispatcher: Dispatcher<AmmSink>) {
        match Logger::install(Box::new(dispatcher)) {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        match TracingBridge::install_log_capture() {
            Ok(()) | Err(InstallError::AlreadyInstalled) => {}
        }
        // The only failure is a global subscriber already set in this process, which counts as installed
        if tracing_subscriber::registry().with(TracingBridge::to_global()).try_init().is_err() {
            curia::debug!("tracing subscriber already set, kept as it is");
        }
    }

    // Deletes the oldest sessions of every mod folder under the log root, never the current one.
    fn prune(root: &LogRoot, keep: usize, stamp: &SessionStamp) {
        let Ok(entries) = fs::read_dir(root.root()) else {
            return;
        };
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            if let Err(error) = LogRetention::prune_keeping(&dir, keep, stamp) {
                curia::warn!("old session logs could not be deleted", { error: error.to_string() });
            }
        }
    }

    fn fields(json: String) -> Fields {
        if json.trim().is_empty() {
            return Fields::new();
        }
        match serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&json) {
            Ok(map) => Fields::from_map(map),
            Err(_) => {
                let mut fields = Fields::new();
                fields.insert(Self::FIELDS_UNPARSED, json);
                fields
            }
        }
    }

    fn level(level: AmmLogLevel) -> Level {
        match level {
            AmmLogLevel::Error => Level::Error,
            AmmLogLevel::Warn => Level::Warn,
            AmmLogLevel::Info => Level::Info,
            AmmLogLevel::Debug => Level::Debug,
            AmmLogLevel::Trace => Level::Trace,
        }
    }

    fn abi_level(level: Level) -> AmmLogLevel {
        match level {
            Level::Error => AmmLogLevel::Error,
            Level::Warn => AmmLogLevel::Warn,
            Level::Info => AmmLogLevel::Info,
            Level::Debug => AmmLogLevel::Debug,
            Level::Trace => AmmLogLevel::Trace,
        }
    }

    fn cell() -> &'static OnceLock<LogBridge> {
        static BRIDGE: OnceLock<LogBridge> = OnceLock::new();
        &BRIDGE
    }
}
