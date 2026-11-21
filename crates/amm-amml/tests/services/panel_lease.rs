use std::sync::{Arc, Mutex};

use amml_api::amml_abi::{AmmlReleaseReason, InjectedInputReleased};
use amml_api::testing::FakeGame;
use amml_api::{HookOrder, InputBlocks, KeyCode, LeaseMode, LeaseRequest, LeaseState};
use amml_modmanager::services::PanelLease;

use crate::support::ManagerRig;

const SHARED: LeaseRequest =
    LeaseRequest { mode: LeaseMode::Shared, blocks: InputBlocks::NONE, passthrough: &[], preempt: false };

#[test]
fn panel_open_suspends_a_consumer_lease_and_releases_its_keys() {
    let rig = ManagerRig::new();
    let released: Arc<Mutex<Vec<(u64, AmmlReleaseReason)>>> = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&released);
    let listener = rig
        .fake
        .ctx()
        .events()
        .listen::<InjectedInputReleased>(HookOrder::NORMAL, move |event, _| {
            seen.lock().unwrap().push((event.lease, event.reason));
        })
        .unwrap();
    rig.start();
    let lease = rig.fake.ctx().input().acquire(SHARED).unwrap();
    let hold = lease.key_down(KeyCode::W).unwrap();
    rig.fake.frame();

    rig.toggle_panel();
    rig.fake.frame();
    let suspended = lease.state();
    let after_open = rig.fake.injected();
    rig.toggle_panel();
    rig.fake.frame();

    assert_eq!(after_open, vec!["key 0x57 down".to_string(), "key 0x57 up".to_string()]);
    assert_eq!(suspended, LeaseState::Suspended);
    assert_eq!(*released.lock().unwrap(), vec![(lease.id(), AmmlReleaseReason::SUSPENDED)]);
    assert_eq!(lease.state(), LeaseState::Active);
    assert_eq!(rig.fake.injected(), after_open);
    assert!(!rig.panel_lease_held());
    drop(hold);
    drop(listener);
}

#[test]
fn the_panel_lease_blocks_every_real_key_and_ends_on_drop() {
    let fake = FakeGame::new();
    let input = fake.ctx().input();
    let other = input.acquire(SHARED).unwrap();

    let panel = PanelLease::open(&input).unwrap();
    let blocked = !fake.real_key(KeyCode::W.0, true);
    let other_while_open = other.state();
    drop(panel);

    assert!(blocked);
    assert_eq!(other_while_open, LeaseState::Suspended);
    assert_eq!(other.state(), LeaseState::Active);
}

#[test]
fn a_second_panel_lease_is_refused_while_one_preempts() {
    let fake = FakeGame::new();
    let input = fake.ctx().input();
    let first = PanelLease::open(&input).unwrap();

    let second = PanelLease::open(&input);

    assert_eq!(first.state(), LeaseState::Active);
    assert!(second.is_err());
}
