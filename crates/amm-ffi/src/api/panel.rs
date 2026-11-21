use crate::services::{Boundary, HostCell};

/// Core table `panel_open`, any thread. It reads the lock-free host state, so it never waits on the host lock. 0
/// with no host or a disabled manager.
pub extern "C" fn amm_panel_open() -> u8 {
    let result = Boundary::table(|| Ok(HostCell::global().shared().is_some_and(|shared| shared.panel_open())));
    u8::from(result.unwrap_or(false))
}
