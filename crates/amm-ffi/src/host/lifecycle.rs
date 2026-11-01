use std::path::PathBuf;

use amm_abi::types::AmmStatus;

use crate::services::{Boundary, HostCell};

/// Starts the manager: reads its config from `config_dir`, opens the session logs under `game_dir/logs/amm`, and
/// starts the config writer. `version` is the manager version the panel shows. A second call keeps the running
/// host. It creates threads, so call it from the mod's enable, never under the OS loader lock.
///
/// # Safety
///
/// Each text pointer is null or holds its length in readable UTF-8 bytes for the call.
pub unsafe extern "C" fn amm_host_create(
    config_dir: *const u8,
    len: usize,
    game_dir: *const u8,
    glen: usize,
    version: *const u8,
    vlen: usize,
) -> AmmStatus {
    Boundary::status(Boundary::guard(|| {
        // SAFETY: the caller gives len readable bytes at config_dir for the call
        let config_dir = PathBuf::from(unsafe { Boundary::text(config_dir, len) }?);
        // SAFETY: the caller gives glen readable bytes at game_dir for the call
        let game_dir = PathBuf::from(unsafe { Boundary::text(game_dir, glen) }?);
        // SAFETY: the caller gives vlen readable bytes at version for the call
        let version = unsafe { Boundary::text(version, vlen) }?.to_string();
        HostCell::global().create(config_dir, game_dir, version)
    }))
}

/// Stops the manager after the mod unhooked: pending config writes are flushed within a bound and the log files
/// are closed. Safe to call when no host runs.
pub extern "C" fn amm_host_destroy() {
    let _ = Boundary::guard(|| {
        HostCell::global().destroy();
        Ok(())
    });
}
