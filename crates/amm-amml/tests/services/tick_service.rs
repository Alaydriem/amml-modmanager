use amml_api::Screen;
use amml_api::testing::FakeClient;
use amml_modmanager::services::{ManagerRuntime, TickService};

use crate::support::ManagerRig;

fn menu_after_tick(rig: &ManagerRig, top_screen: Screen) -> u8 {
    rig.fake.set_client(FakeClient { top_screen, ..ManagerRig::in_play() });
    rig.tick();
    ManagerRuntime::get().and_then(ManagerRuntime::state).map(|state| state.menu_screen()).unwrap()
}

#[test]
fn the_menu_screen_follows_the_top_screen() {
    let rig = ManagerRig::new();
    rig.start();

    assert_eq!(menu_after_tick(&rig, Screen::Start), TickService::MENU_START);
    assert_eq!(menu_after_tick(&rig, Screen::Pause), TickService::MENU_PAUSE);
    assert_eq!(menu_after_tick(&rig, Screen::Other), TickService::MENU_NONE);
}

#[test]
fn the_mouse_is_written_only_in_play() {
    let rig = ManagerRig::new();
    rig.start();
    rig.fake.set_client(FakeClient { local_player: false, world_active: false, ..ManagerRig::in_play() });
    rig.toggle_panel();
    rig.tick();
    let on_the_title_screen = rig.fake.client_writes();

    rig.fake.set_client(ManagerRig::in_play());
    rig.tick();

    assert!(on_the_title_screen.is_empty());
    assert_eq!(rig.fake.client_writes(), vec!["mouse_captured=false".to_string()]);
}

#[test]
fn a_tick_without_a_client_writes_nothing() {
    let rig = ManagerRig::new();
    rig.start();
    rig.toggle_panel();

    rig.fake.raise(&mut amml_api::amml_abi::ClientTick {
        header: amml_api::testing::FakeGame::header::<amml_api::amml_abi::ClientTick>(),
    });

    assert!(rig.fake.client_writes().is_empty());
    assert!(rig.panel_lease_held());
}
