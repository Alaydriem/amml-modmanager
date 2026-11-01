use amm_core::structs::input::FakeKeyOp;

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::{Call, RecordingCalls};

#[test]
fn tick_flushes_after_host_tick() {
    let (state, calls) = InstalledRuntime::install();
    state.fake_keys().note(0x7000, 1);
    calls.queue_fake_keys(&[FakeKeyOp { vk: 0x57, down: true }]);
    let runtime = amml_modmanager::services::ManagerRuntime::get().unwrap();

    runtime.tick(RecordingCalls::CLIENT);

    let seen = calls.take();
    assert_eq!(seen[0], Call::HostTick(0));
    assert_eq!(seen[1], Call::GameKey { controller: 0x7000, vk: 0x57, down: true, origin: 1 });
}
