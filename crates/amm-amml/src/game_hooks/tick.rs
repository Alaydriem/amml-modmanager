use amml_sdk::amml_detour;

use crate::services::ManagerRuntime;

// bool ClientInstance::update(bool isInitFinished) (26.60: update(bool, bool)): once per client tick on the game thread; the manager ticks
// after the game's own update
// 26.60 added a bool third argument (r8). It is carried as a raw u64 so the register reaches the original
// bit for bit; 26.51 ignores r8, so one declaration works on both builds
amml_detour!(pub mod client_update(this: usize, init_finished: u8, extra: u64) -> u8 |original| {
    let result = original(this, init_finished, extra);
    if let Some(runtime) = ManagerRuntime::get() {
        runtime.tick(this);
    }
    result
});
