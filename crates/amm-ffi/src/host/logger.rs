use amm_abi::types::{AmmLogLevel, AmmStatus};
use amm_store::logging::ModLogRouter;

use crate::services::{Boundary, HostCell, LogBridge};
use crate::types::AmmHostConsoleCallback;

/// Sets the LeviLamina console mirror. Warn and Error lines queued since the last tick are given to it on each
/// `amm_host_tick`. Null stops the mirror.
pub extern "C" fn amm_host_set_console(cb: Option<unsafe extern "C" fn(level: u8, msg: *const u8, len: usize)>) {
    let _ = Boundary::guard(|| {
        let cb: Option<AmmHostConsoleCallback> = cb;
        HostCell::global().set_console(cb);
        Ok(())
    });
}

/// One log event of the manager mod, written to the manager's own session file. `level` is an `AmmLogLevel`
/// value; any other value drops the event, as does a call before the first host was created.
///
/// # Safety
///
/// Each text pointer is null or holds its length in readable bytes for the call.
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn amm_host_log(
    level: u8,
    target: *const u8,
    tlen: usize,
    msg: *const u8,
    mlen: usize,
    fields_json: *const u8,
    flen: usize,
    file: *const u8,
    flen2: usize,
    line: u32,
) {
    let _ = Boundary::guard(|| {
        let level = AmmLogLevel::try_from(level)?;
        let logs = LogBridge::current().ok_or(AmmStatus::ManagerMissing)?;
        // SAFETY: the caller gives each text pointer with its length in readable bytes for the call
        let parts = unsafe {
            [
                Boundary::lossy_text(target, tlen),
                Boundary::lossy_text(msg, mlen),
                Boundary::lossy_text(fields_json, flen),
                Boundary::lossy_text(file, flen2),
            ]
        };
        logs.emit(ModLogRouter::MANAGER_SLOT, level, parts, line);
        Ok(())
    });
}
