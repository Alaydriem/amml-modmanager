use amm_core::structs::clash::ClashKind;
use amm_core::structs::event::{KeybindPhase, ModEvent};

use crate::support::{Descriptors, HostFixture, SimMod};

const KEY_K: u32 = 0x4B;

fn key_clashes(fixture: &HostFixture) -> usize {
    fixture.with_runtime(|runtime| {
        let report = runtime.manager().clash_report();
        report.conflicts.iter().filter(|clash| matches!(clash.kind, ClashKind::Key { .. })).count()
    })
}

#[test]
fn two_global_binds_on_one_chord_clash_and_a_rebind_removes_the_clash() {
    let fixture = HostFixture::new();
    let bind = Descriptors::chord_bind("light", "Hud", "Ctrl+L");
    let mut lite = SimMod::new(&fixture, Descriptors::with_claims("lite", &[("Hud", "Shared")], &bind), "");
    let mut lamp = SimMod::new(&fixture, Descriptors::with_claims("lamp", &[("Hud", "Shared")], &bind), "");
    lite.drain();
    lamp.drain();

    let before = key_clashes(&fixture);
    let rebound = lamp.link().set_setting("keys.light", &hcl::Value::String("Ctrl+K".to_owned()));
    let after = key_clashes(&fixture);

    assert_eq!(before, 1);
    assert!(rebound.is_ok());
    assert_eq!(after, 0);

    fixture.key(KEY_K, true, HostFixture::CTRL, 100);
    fixture.key(KEY_K, false, HostFixture::CTRL, 110);

    assert!(lamp.drain().contains(&ModEvent::Keybind { action: 0, phase: KeybindPhase::Pressed }));
    assert!(lite.drain().iter().all(|event| !matches!(event, ModEvent::Keybind { .. })));
}
