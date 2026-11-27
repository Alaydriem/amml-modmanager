use amm_core::config::{ExclusiveResolution, PairKey, SuspensionResolution};
use amm_core::services::settings::ManagerConfigFile;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::settings::HclDocument;
use amm_ffi::services::HostCell;
use amml_api::{ApiError, InputBlocks, LeaseMode, LeaseRequest, LeaseState};
use amml_modmanager::ManagerMod;

use crate::support::{HighMod, LowMod, ManagerRig};

const EXCLUSIVE: LeaseRequest =
    LeaseRequest { mode: LeaseMode::Exclusive, blocks: InputBlocks::NONE, passthrough: &[], preempt: false };

fn prefer(preferred: &str) {
    let key = PairKey::new(Capability::Input, "@test/high", "@test/low");
    let result = HostCell::global().with(|runtime| {
        runtime.manager_mut().set_exclusive_resolution(key, ExclusiveResolution::Prefer(preferred.to_string()));
        Ok(())
    });
    assert_eq!(result, Ok(()));
}

// The manager's config file names `preferred` for the pair, as the panel saves it; the host reads it when the
// enable starts it.
fn save_preference(rig: &ManagerRig, preferred: &str) {
    let key = PairKey::new(Capability::Input, "@test/high", "@test/low");
    let resolution = ExclusiveResolution::Prefer(preferred.to_string());
    let document = ManagerConfigFile::with_exclusive(&HclDocument::empty(), &key, &resolution);
    std::fs::create_dir_all(rig.config_dir()).unwrap();
    std::fs::write(rig.config_dir().join("config.hcl"), document.text()).unwrap();
}

#[test]
fn ranking_is_set_at_enable_and_on_change() {
    let rig = ManagerRig::new();
    save_preference(&rig, "@test/high");
    rig.fake.load::<ManagerMod>().unwrap();
    rig.fake.enable::<ManagerMod>().unwrap();
    rig.fake.open_gate();
    rig.fake.set_client(ManagerRig::in_play());
    let (high, low) = (rig.fake.ctx_for::<HighMod>().input(), rig.fake.ctx_for::<LowMod>().input());

    // ranked above `low`, `high` takes its exclusive lease from it; unranked, the second would be refused
    let low_first = low.acquire(EXCLUSIVE).unwrap();
    let high_second = high.acquire(EXCLUSIVE).unwrap();
    let at_enable = (low_first.state(), high_second.state());
    drop((low_first, high_second));
    prefer("@test/low");
    rig.tick();
    let high_first = high.acquire(EXCLUSIVE).unwrap();
    let low_second = low.acquire(EXCLUSIVE).unwrap();

    assert_eq!(at_enable, (LeaseState::Suspended, LeaseState::Active));
    assert_eq!((high_first.state(), low_second.state()), (LeaseState::Suspended, LeaseState::Active));
    assert_eq!(rig.log_count("info lease ranking set mods=2"), 2);
}

#[test]
fn without_a_resolution_the_first_exclusive_lease_wins() {
    let rig = ManagerRig::new();
    rig.start();
    let (high, low) = (rig.fake.ctx_for::<HighMod>().input(), rig.fake.ctx_for::<LowMod>().input());

    let low_first = low.acquire(EXCLUSIVE).unwrap();
    let high_second = high.acquire(EXCLUSIVE);

    assert_eq!(low_first.state(), LeaseState::Active);
    assert_eq!(high_second.err(), Some(ApiError::Refused));
    assert_eq!(rig.log_count("info lease ranking set mods=0"), 1);
}

const SHARED: LeaseRequest =
    LeaseRequest { mode: LeaseMode::Shared, blocks: InputBlocks::NONE, passthrough: &[], preempt: false };

fn coexist() {
    let key = PairKey::new(Capability::Input, "@test/high", "@test/low");
    let result = HostCell::global().with(|runtime| {
        runtime.manager_mut().set_suspension_resolution(key, SuspensionResolution::Coexist);
        Ok(())
    });
    assert_eq!(result, Ok(()));
}

#[test]
fn an_exclusive_lease_suspends_a_shared_one_until_the_panel_sets_the_pair_to_coexist() {
    let rig = ManagerRig::new();
    rig.start();
    let (high, low) = (rig.fake.ctx_for::<HighMod>().input(), rig.fake.ctx_for::<LowMod>().input());
    let shared = low.acquire(SHARED).unwrap();

    let exclusive = high.acquire(EXCLUSIVE).unwrap();
    let before = (shared.state(), exclusive.state());
    coexist();
    rig.tick();
    let after = (shared.state(), exclusive.state());
    drop(exclusive);

    assert_eq!(before, (LeaseState::Suspended, LeaseState::Active));
    assert_eq!(after, (LeaseState::Active, LeaseState::Active));
    assert_eq!(shared.state(), LeaseState::Active);
    assert_eq!(rig.log_count("info lease coexist pairs set pairs=1"), 1);
}
