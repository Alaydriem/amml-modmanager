use amm_ffi::services::HostCell;
use amml_api::amml_abi::Shutdown;
use amml_api::testing::FakeGame;
use amml_modmanager::ManagerMod;

use crate::support::{Descriptors, ManagerRig};

#[test]
fn load_starts_no_host_and_enable_starts_it_before_the_tables() {
    let rig = ManagerRig::new();

    rig.fake.load::<ManagerMod>().unwrap();
    let after_load = HostCell::global().shared().is_some();
    rig.fake.enable::<ManagerMod>().unwrap();

    assert!(!after_load);
    assert!(HostCell::global().shared().is_some());
    assert!(rig.fake.ctx().registry().require::<amm_abi::types::AmmCoreTable>().is_some());
    assert_eq!(rig.log_count("info manager enabled"), 1);
}

#[test]
fn disable_stops_the_host_and_writes_no_client_flag_itself() {
    let rig = ManagerRig::new();
    rig.start();
    let hud = rig.register(&Descriptors::hud("hud"));
    rig.acquire_hud(hud);
    rig.toggle_panel();
    rig.tick();
    let before = rig.fake.client_writes();

    rig.fake.disable::<ManagerMod>();

    assert_eq!(before, vec!["hide_hud=true".to_string(), "mouse_captured=false".to_string()]);
    assert_eq!(rig.fake.client_writes(), before);
    assert!(HostCell::global().shared().is_none());
    assert!(HostCell::global().tables_closed());
}

#[test]
fn shutdown_ends_the_panel_lease_while_the_game_runs() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();
    assert!(rig.panel_lease_held());

    rig.fake.raise_with_client(&mut Shutdown { header: FakeGame::header::<Shutdown>() });

    assert!(!rig.panel_lease_held());
    assert_eq!(rig.log_count("info loader shutdown seen game_alive=1"), 1);
}
