use std::sync::atomic::{AtomicU64, Ordering};

use crossbeam_channel::{Receiver, Sender, TrySendError, bounded};
use curia::{Level, LogEvent, Sink};

use crate::logging::LineFormat;

/// The sending end of the console line channel: the level and the rendered line.
pub type ConsoleSender = Sender<(Level, String)>;
/// The receiving end of the console line channel.
pub type ConsoleReceiver = Receiver<(Level, String)>;

/// Queues Warn and Error lines for the game console. The channel holds `CAPACITY` lines; a line past that is
/// dropped and counted. The reader of the channel passes each line to the game.
pub struct GameConsoleSink {
    tx: ConsoleSender,
    dropped: AtomicU64,
}

impl GameConsoleSink {
    pub const CAPACITY: usize = 256;

    pub fn new(tx: ConsoleSender) -> GameConsoleSink {
        Self { tx, dropped: AtomicU64::new(0) }
    }

    /// A channel of `CAPACITY` lines to build a sink from.
    pub fn channel() -> (ConsoleSender, ConsoleReceiver) {
        bounded(Self::CAPACITY)
    }

    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Sink for GameConsoleSink {
    fn level(&self) -> Level {
        Level::Warn
    }

    fn emit(&self, event: &LogEvent) {
        if event.level > Level::Warn {
            return;
        }
        match self.tx.try_send((event.level, LineFormat::render(event))) {
            Ok(()) => {}
            Err(TrySendError::Full(_) | TrySendError::Disconnected(_)) => {
                self.dropped.fetch_add(1, Ordering::Relaxed);
            }
        }
    }
}
