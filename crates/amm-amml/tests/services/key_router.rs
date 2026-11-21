use amml_api::amml_abi::KeyInput;
use amml_api::{Cancel, HookOrder, InputBlocks, KeyCode, LeaseMode, LeaseRequest};

use crate::support::{Descriptors, ManagerRig};

// The phase of a pressed key bind in a `Keybind` event.
const PRESSED: u8 = 0;
const RELEASED: u8 = 1;

#[test]
fn manager_chord_opens_and_closes_the_panel() {
    let rig = ManagerRig::new();
    rig.start();

    rig.toggle_panel();
    let opened = rig.panel_open();
    rig.toggle_panel();

    assert_eq!(opened, 1);
    assert_eq!(rig.panel_open(), 0);
}

#[test]
fn the_chord_key_is_kept_from_the_game() {
    let rig = ManagerRig::new();
    rig.start();
    rig.key(ManagerRig::CTRL, true);
    rig.key(ManagerRig::SHIFT, true);

    let game_got_f10 = rig.key(ManagerRig::F10, true);

    assert!(!game_got_f10);
    assert_eq!(rig.panel_open(), 1);
}

#[test]
fn panel_keys_are_cancelled_for_the_game() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();

    let dispatch = rig.raise_key(KeyCode::W, true);

    assert!(dispatch.cancelled);
}

#[test]
fn keys_pass_to_the_game_while_the_panel_is_closed() {
    let rig = ManagerRig::new();
    rig.start();

    let dispatch = rig.raise_key(KeyCode::W, true);

    assert!(!dispatch.cancelled);
}

#[test]
fn an_f3_chord_fires_with_modifiers_from_input() {
    let rig = ManagerRig::new();
    // F3 goes down before the manager listens: only the loader's key table knows it is held
    rig.key(KeyCode::F3.0, true);
    rig.start();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "F3+G"));

    let game_got_g = rig.key(ManagerRig::G, true);

    assert!(!game_got_g);
    assert_eq!(rig.keybinds(consumer), vec![(0, PRESSED)]);
}

#[test]
fn a_side_modifier_matches_a_generic_chord() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "Ctrl+G"));

    rig.key(ManagerRig::LCONTROL, true);
    rig.key(ManagerRig::LSHIFT, true);
    rig.key(ManagerRig::F10, true);
    let opened = rig.panel_open();
    rig.key(ManagerRig::F10, false);
    rig.key(ManagerRig::LSHIFT, false);
    rig.toggle_panel();
    rig.chord(&[ManagerRig::LCONTROL], ManagerRig::G);

    assert_eq!(opened, 1);
    assert_eq!(rig.keybinds(consumer), vec![(0, PRESSED), (0, RELEASED)]);
}

#[test]
fn a_cancelled_key_reaches_the_manager_chord_but_never_a_bind() {
    let rig = ManagerRig::new();
    // another mod cancels every key before the manager sees it (same band, added first)
    let cancels = rig.fake.ctx().events().listen::<KeyInput>(HookOrder::FIRST, |event, _| event.cancel()).unwrap();
    rig.start();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "Ctrl+G"));

    rig.chord(&[ManagerRig::CTRL], ManagerRig::G);
    let binds = rig.keybinds(consumer);
    rig.toggle_panel();

    assert!(binds.is_empty());
    assert_eq!(rig.panel_open(), 1);
    drop(cancels);
}

#[test]
fn an_injected_key_is_not_routed() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "G"));
    let request = LeaseRequest { mode: LeaseMode::Shared, blocks: InputBlocks::NONE, passthrough: &[], preempt: false };
    let lease = rig.fake.ctx().input().acquire(request).unwrap();

    let hold = lease.key_down(KeyCode(ManagerRig::G)).unwrap();
    rig.fake.frame();

    assert_eq!(rig.fake.injected(), vec!["key 0x47 down".to_string()]);
    assert!(rig.keybinds(consumer).is_empty());
    drop(hold);
}
