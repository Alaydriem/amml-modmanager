use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void GameControllerHandler_GameCore::refresh(): the game's only gamepad poll, which reads GameInput gamepad
// readings and queues the button, trigger and stick changes. It is skipped while the host says a full-block holder
// is active; the game keeps the last state it read, so the first poll after the block queues only the changes since
// then
amml_detour!(pub mod gamepad_refresh(this: usize) -> () |original| {
    let blocked = ManagerRuntime::get().is_some_and(|runtime| runtime.blocks_gamepad());
    if !blocked {
        original(this);
    }
});
