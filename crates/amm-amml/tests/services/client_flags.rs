use std::sync::{Arc, Mutex};

use amml_api::amml_abi::{ClientTick, MouseGrab};
use amml_api::testing::FakeGame;
use amml_api::{Area, HookOrder};
use amml_modmanager::services::ClientFlags;
use amml_modmanager::types::FlagWant;

use crate::support::{Descriptors, ManagerRig};

#[test]
fn hide_hud_follows_a_hud_lease_with_hide_vanilla_hud() {
    let rig = ManagerRig::new();
    rig.start();
    let hud = rig.register(&Descriptors::hud("hud"));

    assert!(rig.acquire_hud(hud));
    rig.tick();
    rig.tick();
    let while_held = rig.fake.client_writes();
    rig.release_hud(hud);
    rig.tick();
    rig.tick();

    assert_eq!(while_held, vec!["hide_hud=true".to_string()]);
    assert_eq!(rig.fake.client_writes(), vec!["hide_hud=true".to_string(), "hide_hud=false".to_string()]);
}

#[test]
fn the_players_own_hidden_hud_is_kept_after_the_lease() {
    let rig = ManagerRig::new();
    rig.start();
    rig.fake.set_client(amml_api::testing::FakeClient { hide_hud: true, ..ManagerRig::in_play() });
    let hud = rig.register(&Descriptors::hud("hud"));

    rig.acquire_hud(hud);
    rig.tick();
    rig.release_hud(hud);
    rig.tick();

    assert_eq!(rig.fake.client_writes(), vec!["hide_hud=true".to_string(), "hide_hud=true".to_string()]);
}

#[test]
fn mouse_is_released_while_the_panel_is_open() {
    let rig = ManagerRig::new();
    rig.start();

    rig.toggle_panel();
    rig.tick();
    let grab = rig.fake.raise_with_client(&mut MouseGrab { header: FakeGame::header::<MouseGrab>() });
    rig.toggle_panel();
    rig.tick();
    let grab_after = rig.fake.raise_with_client(&mut MouseGrab { header: FakeGame::header::<MouseGrab>() });

    assert!(grab.cancelled);
    assert!(!grab_after.cancelled);
    assert_eq!(rig.fake.client_writes(), vec!["mouse_captured=false".to_string(), "mouse_captured=true".to_string()]);
}

#[test]
fn the_mouse_is_left_to_the_game_in_its_menus() {
    let rig = ManagerRig::new();
    rig.start();
    rig.fake.set_client(amml_api::testing::FakeClient { showing_menu: true, ..ManagerRig::in_play() });

    rig.toggle_panel();
    rig.tick();

    assert!(rig.fake.client_writes().is_empty());
}

#[test]
fn a_missing_hud_area_turns_off_only_hud_hide() {
    let rig = ManagerRig::new();
    rig.fake.withdraw_area(Area::CLIENT_HUD);
    rig.start();
    let hud = rig.register(&Descriptors::hud("hud"));

    rig.acquire_hud(hud);
    rig.toggle_panel();
    rig.tick();
    rig.tick();

    assert_eq!(rig.log_count("warn feature off feature=hud area=client.hud"), 1);
    assert_eq!(rig.log_count("warn feature off"), 1);
    assert_eq!(rig.fake.client_writes(), vec!["mouse_captured=false".to_string()]);
}

#[test]
fn a_not_ready_write_keeps_the_flag_and_is_tried_again_at_the_next_tick() {
    let fake = FakeGame::new();
    fake.offer::<ClientTick>();
    fake.offer_area(Area::CLIENT_HUD);
    fake.offer_area(Area::CLIENT_MOUSE);
    fake.set_client(ManagerRig::in_play());
    let ctx = fake.ctx();
    let flags = Arc::new(Mutex::new(ClientFlags::new(ctx.log(), true, true)));
    let ticked = Arc::clone(&flags);
    let want = FlagWant { hide_hud: true, mouse_free: false, in_play: true };
    let listener = ctx
        .events()
        .listen::<ClientTick>(HookOrder::NORMAL, move |_, at| {
            if let Some(client) = at.client() {
                ticked.lock().unwrap().apply(&client, want);
            }
        })
        .unwrap();

    // the game gate is closed, so every client call gives `NotReady`
    fake.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() });
    fake.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() });

    let writes_while_closed = fake.client_writes();
    fake.open_gate();
    fake.raise_with_client(&mut ClientTick { header: FakeGame::header::<ClientTick>() });

    let lines: Vec<String> = fake.logs().into_iter().filter(|line| line.starts_with("warn")).collect();
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("warn client flag refused feature=hud area=client.hud"), "{lines:?}");
    assert!(!flags.lock().unwrap().hud_off());
    assert!(!flags.lock().unwrap().mouse_off());
    assert!(writes_while_closed.is_empty());
    assert_eq!(fake.client_writes(), vec!["hide_hud=true".to_string()]);
    drop(listener);
}
