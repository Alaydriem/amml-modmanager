use crate::services::{Boundary, HostCell};

/// Game thread, each tick. Hands the due config writes to the writer, logs telemetry when it is on, and gives
/// the queued console lines to the console mirror.
pub extern "C" fn amm_host_tick(now_ms: u64) {
    let _ = Boundary::guard(|| HostCell::global().tick(now_ms));
}
