use crate::services::{Boundary, HostCell};

/// From the gamepad hook, on the thread that polls the gamepad. True while the active exclusive Input holder has a
/// `full_block` claim. It reads lock-free state; a failure answers false.
pub extern "C" fn amm_host_blocks_gamepad() -> bool {
    Boundary::guard(|| Ok(HostCell::global().blocks_gamepad())).unwrap_or(false)
}
