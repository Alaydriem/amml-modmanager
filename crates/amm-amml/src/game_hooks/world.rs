use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void ClientInstance::onPlayerLoaded(Player& player), the function LeviLamina raises ClientJoinLevelEvent from;
// like that event, the manager hears of it before the original runs
amml_detour!(pub mod player_loaded(this: usize, player: usize) -> () |original| {
    if let Some(runtime) = ManagerRuntime::get() {
        runtime.world_loaded();
    }
    original(this, player)
});

// void ClientInstance::onLevelExit(), the function LeviLamina raises ClientExitLevelEvent from; the manager hears of
// it before the original runs. `this` is the GameCallbacks subobject (ClientInstance + 0x88), passed on unchanged
amml_detour!(pub mod level_exit(this: usize) -> () |original| {
    if let Some(runtime) = ManagerRuntime::get() {
        runtime.world_left();
    }
    original(this)
});
