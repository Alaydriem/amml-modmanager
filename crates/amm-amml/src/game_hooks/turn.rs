use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void LocalPlayer::_applyTurnDelta(Vec2 const& turnOffset): mouse look reaches the player here; dropped while the
// host says a mod that holds Input blocks the player's own turn. The Vec2 reference is passed on unchanged
amml_detour!(pub mod apply_turn_delta(this: usize, delta: usize) -> () |original| {
    let blocked = ManagerRuntime::get().is_some_and(|runtime| runtime.blocks_turn());
    if !blocked {
        original(this, delta);
    }
});
