use crate::services::{Boundary, HostCell};

/// Game thread, from the turn hook. True while a mod that holds `Input` and is not suspended asks for the
/// player's own turn to be blocked. It reads lock-free state; a failure answers false.
pub extern "C" fn amm_host_blocks_player_turn() -> bool {
    Boundary::guard(|| Ok(HostCell::global().blocks_player_turn())).unwrap_or(false)
}
