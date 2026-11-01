use amm_abi::types::{AmmHandle, AmmLogLevel, AmmStatus};

use crate::services::{Boundary, HostCell, LogBridge};

/// Table `log`. It never takes the host lock. A bad level byte, a stale handle or no host drops the event.
///
/// # Safety
///
/// Each text pointer is null or holds its length in readable bytes for the call.
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn amm_log(
    handle: AmmHandle,
    level: u8,
    target: *const u8,
    target_len: usize,
    message: *const u8,
    message_len: usize,
    fields_json: *const u8,
    fields_len: usize,
    file: *const u8,
    file_len: usize,
    line: u32,
) {
    let _ = Boundary::guard(|| {
        let level = AmmLogLevel::try_from(level)?;
        let shared = HostCell::global().shared().ok_or(AmmStatus::ManagerMissing)?;
        if !shared.is_live(handle) {
            return Err(AmmStatus::UnknownHandle);
        }
        let slot = u8::try_from(handle.slot).map_err(|_| AmmStatus::UnknownHandle)?;
        let logs = LogBridge::current().ok_or(AmmStatus::ManagerMissing)?;
        // SAFETY: the caller gives each text pointer with its length in readable bytes for the call
        let parts = unsafe {
            [
                Boundary::lossy_text(target, target_len),
                Boundary::lossy_text(message, message_len),
                Boundary::lossy_text(fields_json, fields_len),
                Boundary::lossy_text(file, file_len),
            ]
        };
        logs.emit(slot, level, parts, line);
        Ok(())
    });
}
