use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void MinecraftGame::grabMouse(): the game grabs the mouse again on its own while the world shows with no screen.
// `this` is kept as the MinecraftGame the manager releases and grabs through, and the grab is skipped while the
// layers keep the cursor free
amml_detour!(pub mod grab_mouse(this: usize) -> () |original| {
    let blocked = ManagerRuntime::get().is_some_and(|runtime| runtime.grab_called(this));
    if !blocked {
        original(this);
    }
});
