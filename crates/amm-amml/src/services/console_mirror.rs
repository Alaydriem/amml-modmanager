#![allow(unsafe_code)]

use std::panic::{AssertUnwindSafe, catch_unwind};

use amm_abi::types::AmmLogLevel;
use amml_sdk::ModRuntime;
use amml_sdk::amml_abi::AmmlLogLevel;

/// The host's console: the Warn and Error lines the host queues go to the manager's AMML log.
pub struct ConsoleMirror;

impl ConsoleMirror {
    /// The callback `amm_host_set_console` takes. Called by the host on the game thread, inside `amm_host_tick`.
    ///
    /// # Safety
    ///
    /// `msg` is null or holds `len` readable bytes for the call.
    pub unsafe extern "C" fn write(level: u8, msg: *const u8, len: usize) {
        if msg.is_null() {
            return;
        }
        // SAFETY: the caller gives `len` readable bytes at `msg` for the call
        let bytes = unsafe { std::slice::from_raw_parts(msg, len) };
        let _ = catch_unwind(AssertUnwindSafe(|| {
            if let Some(runtime) = ModRuntime::get() {
                runtime.logger().log(Self::level(level), &String::from_utf8_lossy(bytes));
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
