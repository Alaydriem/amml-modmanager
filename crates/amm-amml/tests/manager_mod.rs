use amm_abi::types::{AmmCoreTable, AmmKeybindsTable, AmmLayersTable, AmmLeasesTable, AmmSettingsTable, AmmStatus};
use amml_api::amml_abi::AmmlDirKind;
use amml_api::testing::FakeGame;
use amml_api::{AmmlMod, ApiError};
use amml_modmanager::ManagerMod;
use amml_modmanager::services::ManagerRuntime;
use hcl::Expression;

use crate::support::{Descriptors, ManagerRig};

const MANIFEST: &str = include_str!("../packaging/amml.hcl");

fn attribute(key: &str) -> Expression {
    let body = hcl::parse(MANIFEST).unwrap();
    body.attributes().find(|attribute| attribute.key() == key).unwrap().expr().clone()
}

#[test]
fn the_manifest_names_the_mod_and_its_dll() {
    assert_eq!(attribute("namespace"), Expression::String(ManagerMod::NAMESPACE.to_string()));
    assert_eq!(attribute("name"), Expression::String(ManagerMod::NAME.to_string()));
    assert_eq!(ManagerMod::id(), ManagerMod::MODULE);
    assert_eq!(attribute("entry"), Expression::String("amml_modmanager.dll".to_string()));
}

#[test]
fn enable_provides_the_five_tables_and_listens() {
    let rig = ManagerRig::new();

    rig.start();

    let registry = rig.fake.ctx().registry();
    assert!(registry.require::<AmmCoreTable>().is_some());
    assert!(registry.require::<AmmSettingsTable>().is_some());
    assert!(registry.require::<AmmKeybindsTable>().is_some());
    assert!(registry.require::<AmmLayersTable>().is_some());
    assert!(registry.require::<AmmLeasesTable>().is_some());
    assert!(rig.fake.missing_required().is_empty());
    let runtime = ManagerRuntime::get().unwrap();
    assert_eq!(runtime.listener_count(), 22);
}

#[test]
fn enable_refuses_without_client_ticks() {
    let rig = ManagerRig::without_events();
    rig.offer_all_but_tick();
    rig.fake.load::<ManagerMod>().unwrap();

    let enabled = rig.fake.enable::<ManagerMod>();

    assert_eq!(enabled, Err(ApiError::NotAvailable));
    assert!(rig.fake.ctx().registry().require::<AmmCoreTable>().is_none());
    assert_eq!(ManagerRuntime::get().unwrap().listener_count(), 0);
    assert!(amm_ffi::services::HostCell::global().shared().is_none(), "the host and its writer thread stop");
}

#[test]
fn load_without_a_data_folder_is_not_available_with_a_log_line() {
    let fake = FakeGame::new();
    let dir = tempfile::tempdir().unwrap();
    fake.set_dir(AmmlDirKind::CONFIG, &dir.path().join("config").display().to_string());

    let loaded = fake.load::<ManagerMod>();

    assert_eq!(loaded, Err(ApiError::NotAvailable));
    assert_eq!(fake.logs(), vec!["error manager folders missing error=the loader gives the manager no data folder"]);
    assert!(ManagerRuntime::get().is_none());
}

#[test]
fn disable_ends_the_tables_and_the_panel_lease() {
    let rig = ManagerRig::new();
    rig.start();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "Ctrl+G"));
    // the consumer keeps the tables it required; they stay readable after the manager is disabled
    let (core, settings) = (rig.core(), rig.settings());
    rig.toggle_panel();
    assert_eq!(rig.panel_open(), 1);
    assert!(rig.panel_lease_held());

    rig.fake.disable::<ManagerMod>();

    // SAFETY: panel_open takes no arguments
    assert_eq!(unsafe { (core.panel_open.unwrap())() }, 0);
    let (path, value) = ("keys.act", "\"Ctrl+H\"");
    // SAFETY: both texts live for the call
    let status =
        unsafe { (settings.set_setting.unwrap())(consumer, path.as_ptr(), path.len(), value.as_ptr(), value.len()) };
    assert_eq!(status, AmmStatus::Disabled);
    assert!(rig.fake.ctx().registry().require::<AmmCoreTable>().is_none());
    assert!(!rig.panel_lease_held());
    assert_eq!(ManagerRuntime::get().unwrap().listener_count(), 0);
}

#[test]
fn an_enable_after_a_disable_starts_a_new_host_with_open_tables() {
    let rig = ManagerRig::new();
    rig.start();
    rig.fake.disable::<ManagerMod>();

    rig.fake.enable::<ManagerMod>().unwrap();
    let consumer = rig.register(&Descriptors::with_bind("consumer", "Ctrl+G"));
    rig.toggle_panel();

    assert_eq!(rig.panel_open(), 1);
    assert_eq!(rig.set_setting(consumer, "keys.act", "\"Ctrl+H\""), AmmStatus::Ok);
}

#[test]
fn the_manifest_uses_block_is_the_uses_of_the_mod() {
    let manifest = MANIFEST.replace("\r\n", "\n");

    assert_eq!(manifest.matches(&ManagerMod::USES.to_hcl()).count(), 1);
    assert_eq!(ManagerMod::USES.write_into(&manifest), manifest);
}

#[test]
fn the_manifest_writer_keeps_a_crlf_manifest_and_adds_no_second_block() {
    let crlf = MANIFEST.replace("\r\n", "\n").replace('\n', "\r\n");

    let once = ManagerMod::USES.write_into(&crlf);
    let twice = ManagerMod::USES.write_into(&once);

    assert_eq!(once, crlf);
    assert_eq!(twice, crlf);
    assert_eq!(twice.matches("\nuses {\r\n").count(), 1);
}
