use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use amm_core::structs::input::FakeKeyOp;
use amml_modmanager::game_hooks::{key_down, key_up};

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::Call;

static GAME_KEYS: Mutex<Vec<(i32, bool)>> = Mutex::new(Vec::new());

extern "C" fn game_key_down(_this: usize, key: i32, _origin: i32) {
    GAME_KEYS.lock().unwrap().push((key, true));
}

extern "C" fn game_key_up(_this: usize, key: i32) {
    GAME_KEYS.lock().unwrap().push((key, false));
}

fn originals() {
    key_down::GUARD.original().store(game_key_down as *mut c_void, Ordering::SeqCst);
    key_up::GUARD.original().store(game_key_up as *mut c_void, Ordering::SeqCst);
}

#[test]
fn a_consumed_key_never_reaches_the_game() {
    let (_, calls) = InstalledRuntime::install();
    originals();
    calls.consume_keys.store(true, Ordering::SeqCst);

    key_down::detour(0x7000, 0x41, 1);
    key_up::detour(0x7000, 0x41);

    assert!(GAME_KEYS.lock().unwrap().is_empty());
    assert_eq!(calls.take().len(), 2);
}

#[test]
fn a_key_the_host_leaves_reaches_the_game_unchanged() {
    let (_, calls) = InstalledRuntime::install();
    originals();

    key_down::detour(0x7000, 0xA0, 1);
    key_up::detour(0x7000, 0xA0);

    assert_eq!(*GAME_KEYS.lock().unwrap(), [(0xA0, true), (0xA0, false)]);
    assert_eq!(
        calls.take(),
        vec![
            Call::HostKey { vk: 0x10, down: true, repeat: false },
            Call::HostKey { vk: 0x10, down: false, repeat: false }
        ]
    );
}

#[test]
fn a_real_key_down_sets_where_fake_keys_go() {
    let (state, calls) = InstalledRuntime::install();
    originals();
    calls.queue_fake_keys(&[FakeKeyOp { vk: 0x57, down: true }]);

    key_down::detour(0x7000, 0x41, 3);
    state.fake_keys().flush(calls);

    assert!(calls.take().contains(&Call::GameKey { controller: 0x7000, vk: 0x57, down: true, origin: 3 }));
}
