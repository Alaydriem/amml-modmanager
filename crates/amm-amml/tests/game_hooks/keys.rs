use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};

use amm_core::structs::input::FakeKeyOp;
use amml_modmanager::game_hooks::{controller_update, key_down, key_up};
use amml_modmanager::services::FakeKeySender;

use crate::support::installed_runtime::InstalledRuntime;
use crate::support::recording_calls::Call;

static GAME_KEYS: Mutex<Vec<(i32, bool)>> = Mutex::new(Vec::new());

extern "C" fn game_key_down(_this: usize, key: i32, _origin: i32) {
    GAME_KEYS.lock().unwrap().push((key, true));
}

extern "C" fn game_key_up(_this: usize, key: i32) {
    GAME_KEYS.lock().unwrap().push((key, false));
}

static GAME_UPDATES: AtomicU32 = AtomicU32::new(0);
const ESCAPE: u32 = 0x1B;

extern "C" fn game_update(_this: usize) {
    GAME_UPDATES.fetch_add(1, Ordering::SeqCst);
}

fn originals() {
    key_down::GUARD.original().store(game_key_down as *mut c_void, Ordering::SeqCst);
    key_up::GUARD.original().store(game_key_up as *mut c_void, Ordering::SeqCst);
    controller_update::GUARD.original().store(game_update as *mut c_void, Ordering::SeqCst);
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

#[test]
fn the_controller_is_found_without_a_real_key() {
    let (state, calls) = InstalledRuntime::install();
    originals();
    calls.queue_fake_keys(&[FakeKeyOp { vk: 0x57, down: true }]);

    controller_update::detour(0x7100);
    state.fake_keys().flush(calls);

    assert_eq!(GAME_UPDATES.load(Ordering::SeqCst), 1);
    assert_eq!(
        calls.take(),
        vec![Call::GameKey { controller: 0x7100, vk: 0x57, down: true, origin: FakeKeySender::DEFAULT_ORIGIN }]
    );
    assert_eq!(calls.key_feed(), Some(true));
}

#[test]
fn a_fake_escape_reaches_the_game_with_no_real_key_so_the_pause_screen_can_close() {
    let (state, calls) = InstalledRuntime::install();
    originals();
    calls.queue_fake_keys(&[FakeKeyOp { vk: ESCAPE, down: true }, FakeKeyOp { vk: ESCAPE, down: false }]);

    controller_update::detour(0x7100);
    state.fake_keys().flush(calls);

    let key = |down| Call::GameKey { controller: 0x7100, vk: ESCAPE, down, origin: FakeKeySender::DEFAULT_ORIGIN };
    assert_eq!(calls.take(), vec![key(true), key(false)]);
}
