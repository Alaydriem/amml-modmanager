use amml_modmanager::services::GameHookSet;
use amml_modmanager::types::HookGroup;
use amml_sdk::SdkError;

use crate::support::fake_hook_host::FakeHookHost;

const TICK: &str = "Test.tick";
const WORLD: &str = "Test.world";
const KEYS: &str = "Test.keys";

#[test]
fn a_missing_required_entry_fails_and_installs_nothing() {
    let host = FakeHookHost::new(&[(WORLD, 0x2000)], &[]);
    let specs = [FakeHookHost::spec(WORLD, HookGroup::World, false), FakeHookHost::spec(TICK, HookGroup::Tick, true)];

    let result = GameHookSet::new().install(&host, &specs);

    assert!(matches!(result, Err(SdkError::MissingEntry(id)) if id == TICK));
    assert!(host.hooked_entries().is_empty());
}

#[test]
fn a_missing_optional_entry_logs_one_line_and_the_rest_install() {
    let host = FakeHookHost::new(&[(TICK, 0x1000)], &[]);
    let specs = [FakeHookHost::spec(WORLD, HookGroup::World, false), FakeHookHost::spec(TICK, HookGroup::Tick, true)];

    GameHookSet::new().install(&host, &specs).unwrap();

    assert_eq!(host.hooked_entries(), [TICK]);
    let warnings = host.warnings.lock().unwrap().clone();
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].starts_with(WORLD) && warnings[0].ends_with("the test feature is off"), "{}", warnings[0]);
}

#[test]
fn a_refused_hook_removes_every_hook_already_installed() {
    let host = FakeHookHost::new(&[(TICK, 0x1000), (WORLD, 0x2000), (KEYS, 0x3000)], &[KEYS]);
    let specs = [
        FakeHookHost::spec(TICK, HookGroup::Tick, true),
        FakeHookHost::spec(WORLD, HookGroup::World, false),
        FakeHookHost::spec(KEYS, HookGroup::Input, true),
    ];
    let set = GameHookSet::new();

    assert!(set.install(&host, &specs).is_err());

    let hooked: Vec<_> = host.hooked.lock().unwrap().iter().map(|(_, handle)| *handle).collect();
    assert_eq!(hooked.len(), 2);
    assert_eq!(*host.unhooked.lock().unwrap(), hooked);
    set.remove_all(&host);
    assert_eq!(host.unhooked.lock().unwrap().len(), 2, "nothing is left to remove");
}

#[test]
fn removing_a_group_unhooks_only_that_group() {
    let host = FakeHookHost::new(&[(TICK, 0x1000), (WORLD, 0x2000)], &[]);
    let specs = [FakeHookHost::spec(WORLD, HookGroup::World, false), FakeHookHost::spec(TICK, HookGroup::Tick, true)];
    let set = GameHookSet::new();
    set.install(&host, &specs).unwrap();
    let world = host.hooked.lock().unwrap()[0].1;

    set.remove(&host, HookGroup::World);

    assert_eq!(*host.unhooked.lock().unwrap(), [world]);
}
