use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Mutex, PoisonError};

use amm_abi::types::AmmLogLevel;
use amml_api::Logger;
use amml_api::amml_abi::AmmlLogLevel;

// The mod's AMML logger while the mirror is on. The host calls `write` with no user data, so the logger is kept
// here.
static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

/// The host's console: the Warn and Error lines the host queues go to the manager's AMML log, so they show in
/// `amml/logs/@amml/modmanager.log` beside the loader's own lines.
pub struct ConsoleMirror;

impl ConsoleMirror {
    /// Sends the host's console lines to `logger` from now on.
    pub fn start(logger: Logger) {
        *LOGGER.lock().unwrap_or_else(PoisonError::into_inner) = Some(logger);
    }

    /// Stops the mirror; later lines are dropped.
    pub fn stop() {
        *LOGGER.lock().unwrap_or_else(PoisonError::into_inner) = None;
    }

    /// The callback `HostCell::set_console` takes. Called by the host on the game thread, inside its tick.
    ///
    /// # Safety
    ///
    /// `msg` is null or holds `len` readable bytes for the call.
    #[allow(unsafe_code)]
    pub unsafe extern "C" fn write(level: u8, msg: *const u8, len: usize) {
        if msg.is_null() {
            return;
        }
        // SAFETY: the caller gives `len` readable bytes at `msg` for the call
        let bytes = unsafe { std::slice::from_raw_parts(msg, len) };
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let logger = *LOGGER.lock().unwrap_or_else(PoisonError::into_inner);
            if let Some(logger) = logger {
                logger.log(Self::level(level), &String::from_utf8_lossy(bytes), &[]);
            }
        }));
    }

    /// The AMML level of a host `AmmLogLevel` value; an unknown value logs as Info.
    pub fn level(level: u8) -> AmmlLogLevel {
        match AmmLogLevel::try_from(level) {
            Ok(AmmLogLevel::Error) => AmmlLogLevel::ERROR,
            Ok(AmmLogLevel::Warn) => AmmlLogLevel::WARN,
            Ok(AmmLogLevel::Debug) => AmmlLogLevel::DEBUG,
            Ok(AmmLogLevel::Trace) => AmmlLogLevel::TRACE,
            Ok(AmmLogLevel::Info) | Err(_) => AmmlLogLevel::INFO,
        }
    }
}
