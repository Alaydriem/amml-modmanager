use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// void HIDControllerGameCoreDesktop::onKeyDown(int keyCode, InputOrigin origin): a key reaches the game's
// keyboard feed; a key the host consumes never reaches the original. InputOrigin is an int enum
amml_detour!(pub mod key_down(this: usize, key: i32, origin: i32) -> () |original| {
    if let Some(runtime) = ManagerRuntime::get() {
        runtime.state().fake_keys().note(this, origin);
    }
    let consumed = ManagerRuntime::get().is_some_and(|runtime| runtime.key(key, true));
    if !consumed {
        original(this, key, origin);
    }
});

// void HIDControllerGameCoreDesktop::onKeyUp(int keyCode)
amml_detour!(pub mod key_up(this: usize, key: i32) -> () |original| {
    let consumed = ManagerRuntime::get().is_some_and(|runtime| runtime.key(key, false));
    if !consumed {
        original(this, key);
    }
});
