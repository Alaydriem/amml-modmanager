use std::sync::atomic::Ordering;

use amm_client::link::LinkUses;
use amml_api::{AmmlMod, MoveIntent};
use amml_managerprobe::{ProbeChecks, ProbeLayer, ProbeMod, ProbeStep};

use crate::support::{ChecksMod, ProbeRig};

const MANIFEST: &str = include_str!("../packaging/amml.hcl");

#[test]
fn the_manifest_uses_block_is_the_uses_of_the_mod() {
    let manifest = MANIFEST.replace("\r\n", "\n");

    assert_eq!(manifest.matches(&ProbeMod::USES.to_hcl()).count(), 1);
    assert_eq!(ProbeMod::USES.write_into(&manifest), manifest);
}

#[test]
fn the_manifest_writer_keeps_a_crlf_manifest_and_adds_no_second_block() {
    let crlf = MANIFEST.replace("\r\n", "\n").replace('\n', "\r\n");

    let once = ProbeMod::USES.write_into(&crlf);
    let twice = ProbeMod::USES.write_into(&once);

    assert_eq!(once, crlf);
    assert_eq!(twice, crlf);
    assert_eq!(twice.matches("\nuses {\r\n").count(), 1);
}

#[test]
fn link_uses_are_complete() {
    assert_eq!(LinkUses::check(&ProbeMod::USES), Ok(()));
}

#[test]
fn the_probe_links_and_its_keybind_fires() {
    let rig = ProbeRig::new();
    rig.start();
    rig.mods_loaded();

    rig.tick();
    let before = rig.logged("info probe keybind fired index=0");
    let game_got_f8 = rig.fake.real_key(ProbeRig::F8, true);
    rig.fake.real_key(ProbeRig::F8, false);
    rig.tick();

    assert!(rig.logged("info probe linked"), "{:?}", rig.fake.logs());
    assert!(!before);
    assert!(!game_got_f8);
    assert!(rig.logged("info probe keybind fired index=0"), "{:?}", rig.fake.logs());
}

#[test]
fn the_probe_sees_provider_gone_and_gets_gone() {
    let rig = ProbeRig::new();
    rig.start();
    rig.mods_loaded();
    rig.tick();

    rig.provider_gone();

    let logs = rig.fake.logs();
    let tail: Vec<&str> = logs.iter().rev().take(2).rev().map(String::as_str).collect();
    assert_eq!(tail, ["info probe provider gone", "info probe call after gone result=Gone"], "{logs:?}");
}

#[test]
fn the_checks_go_from_the_layer_to_the_key_hold_through_the_manager() {
    let rig = ProbeRig::new();
    rig.start_manager();
    rig.fake.enable::<ChecksMod>().unwrap();
    let (checks, frames) = ChecksMod::checks().clone().unwrap();

    checks.tick();
    assert_eq!(checks.step(), ProbeStep::Layer);
    checks.tick();
    assert_eq!(checks.step(), ProbeStep::Layer, "the square was not drawn yet");
    frames.store(ProbeLayer::TARGET_FRAMES, Ordering::Relaxed);
    for _ in 0..4 {
        checks.tick();
        rig.tick();
    }
    let logs = rig.fake.logs();

    assert!(logs.contains(&"info probe settings changed".to_owned()), "{logs:?}");
    assert!(logs.contains(&"info probe hud lease granted".to_owned()), "{logs:?}");
    assert_eq!(checks.step(), ProbeStep::KeyHold);
    drop(checks);
    rig.fake.disable::<ChecksMod>();
}

#[test]
fn the_key_hold_waits_for_a_world_then_presses_w_for_forty_ticks_and_lets_go() {
    let rig = ProbeRig::new();
    let checks = rig.checks_at_key_hold();
    checks.tick();
    assert_eq!(checks.step(), ProbeStep::KeyHold);
    assert!(rig.fake.injected().is_empty(), "no world yet, so no hold");

    checks.world_joined();
    checks.move_input(MoveIntent::default());
    rig.settle(&checks);
    checks.tick();
    rig.fake.frame();
    let pressed = rig.fake.injected();
    for _ in 0..ProbeChecks::HOLD_FRAMES {
        checks.tick();
    }
    rig.fake.frame();

    assert_eq!(pressed, ["key 0x57 down"]);
    assert_eq!(rig.fake.injected()[..2], ["key 0x57 down", "key 0x57 up"]);
    assert!(rig.logged("info probe key hold done frames=40 moved=false"), "{:?}", rig.fake.logs());
    assert_eq!(checks.step(), ProbeStep::Move);
    drop(checks);
    rig.fake.disable::<ChecksMod>();
}

#[test]
fn the_key_hold_waits_for_the_game_to_read_movement_after_the_join() {
    let rig = ProbeRig::new();
    let checks = rig.checks_at_key_hold();

    checks.world_joined();
    checks.tick();
    rig.fake.frame();
    let during_loading = rig.fake.injected();
    checks.move_input(MoveIntent::default());
    rig.settle(&checks);
    rig.fake.frame();
    let settling = rig.fake.injected();
    checks.tick();
    rig.fake.frame();

    assert!(during_loading.is_empty(), "{during_loading:?}");
    assert!(settling.is_empty(), "{settling:?}");
    assert_eq!(rig.fake.injected(), ["key 0x57 down"]);
    drop(checks);
    rig.fake.disable::<ChecksMod>();
}

#[test]
fn a_forward_movement_during_the_hold_is_reported_as_moved() {
    let rig = ProbeRig::new();
    let checks = rig.checks_at_key_hold();
    checks.world_joined();
    checks.move_input(MoveIntent::default());
    rig.settle(&checks);
    checks.tick();

    checks.move_input(MoveIntent { forward: 1.0, ..MoveIntent::default() });
    for _ in 0..ProbeChecks::HOLD_FRAMES {
        checks.tick();
    }

    assert!(rig.logged("info probe key hold done frames=40 moved=true"), "{:?}", rig.fake.logs());
    drop(checks);
    rig.fake.disable::<ChecksMod>();
}

#[test]
fn the_key_hold_presses_w_again_once_the_key_feed_is_ready() {
    let rig = ProbeRig::new();
    let checks = rig.checks_at_key_hold_unbound();
    checks.world_joined();
    checks.move_input(MoveIntent::default());
    rig.settle(&checks);
    for _ in 0..ProbeChecks::HOLD_FRAMES {
        checks.tick();
    }
    rig.fake.frame();
    let unbound = rig.fake.injected();

    rig.fake.bind_input();
    checks.tick();
    rig.fake.frame();

    assert!(unbound.is_empty(), "{unbound:?}");
    assert_eq!(checks.step(), ProbeStep::KeyHold);
    assert!(rig.logged("info probe key waits for the key feed"), "{:?}", rig.fake.logs());
    assert_eq!(rig.fake.injected(), ["key 0x57 down"]);
    drop(checks);
    rig.fake.disable::<ChecksMod>();
}
