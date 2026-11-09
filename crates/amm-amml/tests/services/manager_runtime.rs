use std::sync::atomic::Ordering;

use amm_core::structs::input::{FakeKeyOp, PointerButton, PointerOp};

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

#[test]
fn tick_flushes_fake_pointer_ops_after_host_tick() {
    let (state, calls) = InstalledRuntime::install();
    state.fake_pointer().note(0x8000, 5, 6);
    calls.queue_fake_pointer(&[PointerOp::Button { button: PointerButton::Left, down: true }]);
    let runtime = amml_modmanager::services::ManagerRuntime::get().unwrap();

    runtime.tick(RecordingCalls::CLIENT);

    let seen = calls.take();
    assert_eq!(seen[0], Call::HostTick(0));
    assert_eq!(seen[1], Call::GameMouse { device: 0x8000, action: 1, data: 1, x: 5, y: 6 });
}

#[test]
fn tick_replays_an_owed_focus_loss_while_the_window_is_in_the_background() {
    let (_, calls) = InstalledRuntime::install();
    calls.focus_loss_owed.store(true, Ordering::SeqCst);
    calls.foreground.store(false, Ordering::SeqCst);
    let runtime = amml_modmanager::services::ManagerRuntime::get().unwrap();

    runtime.tick(RecordingCalls::CLIENT);
    runtime.tick(RecordingCalls::CLIENT);

    let replays = calls.take().into_iter().filter(|call| *call == Call::ReplayFocusLoss).count();
    assert_eq!(replays, 1);
}

#[test]
fn tick_drops_an_owed_focus_loss_once_the_window_has_focus_again() {
    let (_, calls) = InstalledRuntime::install();
    calls.focus_loss_owed.store(true, Ordering::SeqCst);
    calls.foreground.store(true, Ordering::SeqCst);
    let runtime = amml_modmanager::services::ManagerRuntime::get().unwrap();

    runtime.tick(RecordingCalls::CLIENT);

    assert!(!calls.take().contains(&Call::ReplayFocusLoss));
    assert!(!calls.focus_loss_owed.load(Ordering::SeqCst));
}
