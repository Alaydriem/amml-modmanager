use amm_abi::convert::move_intent::MoveIntentCodec;

use crate::services::{Boundary, HostCell};
use crate::types::AmmHostMove;

/// Game thread, at enable. Whether every game entry of the move command resolved and its hook went in; until it is
/// called with true, `send_move` answers `Unsupported`. Does nothing with no host.
pub extern "C" fn amm_host_set_move_available(available: bool) {
    let _ = Boundary::guard(|| {
        HostCell::global().with(|runtime| {
            runtime.set_move_available(available);
            Ok(())
        })
    });
}

/// Game thread, once per tick. `player_ready` says whether the game has a local player in an active world. True
/// when a mod holds a move intent, written to `out`; false, with nothing written, when none is held, with no host
/// or with a null or unaligned `out`.
///
/// # Safety
///
/// `out` is null or writable for the call.
pub unsafe extern "C" fn amm_host_move_intent(player_ready: bool, out: *mut AmmHostMove) -> bool {
    Boundary::guard(|| {
        let out = Boundary::writable(out)?;
        let intent = HostCell::global().with(|runtime| Ok(runtime.move_tick(player_ready)))?;
        if intent.is_zero() {
            return Ok(false);
        }
        let (forward_milli, left_milli, flags) = MoveIntentCodec::encode(intent);
        // SAFETY: out is non-null and aligned, as checked, and writable for the call, as the caller gives
        unsafe { out.write(AmmHostMove { forward_milli, left_milli, flags }) };
        Ok(true)
    })
    .unwrap_or(false)
}
