use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use amm_abi::types::AmmStatus;
use amm_client::link::{LinkState, ManagerLink};
use amm_client::test_support::{FakeConnection, FakeTable, SettingCall};
use amm_core::errors::LinkError;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent, ReleaseReason};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;
use amml_api::amml_abi::{AmmlReleaseReason, InjectedInputReleased, ScreenshotTaken, ScreenshotWritten};
use amml_api::testing::FakeGame;
use amml_api::{InputBlocks, KeyCode, LeaseMode, LeaseRequest, ShotId, ShotLayers};
use serde::Deserialize;

use crate::support::{DropFlagHandler, IdleHandler};

const INPUT_DESCRIPTOR: &str = "namespace = \"test\"\nname = \"alaydriem-input\"\n\
display_name = \"Input\"\nversion = \"0.1.0\"\nauthor = \"Alaydriem\"\nconfig_path = \"config.hcl\"\n\
claim \"Input\" {\n  mode   = \"Shared\"\n  reason = \"drives the player\"\n}\n";

#[derive(Debug, Deserialize, PartialEq)]
struct Card {
    scale: f32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TestConfig {
    card: Card,
}

// A started link on `FakeTable::game()` with the tables provided but no `ProviderReady` raised yet.
fn waiting() -> FakeConnection {
    let game = FakeTable::game();
    let ctx = FakeTable::ctx_of(&game, FakeTable::DESCRIPTOR);
    let link = match ManagerLink::start_without_frame(&ctx, FakeTable::DESCRIPTOR) {
        Ok(link) => link,
        Err(error) => panic!("start failed: {error}"),
    };
    if let Err(error) = FakeTable::provide(&ctx) {
        panic!("provide failed: {error}");
    }
    FakeConnection { link, game }
}

fn release_of(lease: u64, reason: AmmlReleaseReason) -> InjectedInputReleased {
    let header = FakeGame::header::<InjectedInputReleased>();
    InjectedInputReleased { header, lease, reason, input: InjectedInputReleased::ALL, code: 0, _reserved: 0 }
}

fn taken(id: u64, ok: u8) -> ScreenshotTaken {
    ScreenshotTaken { header: FakeGame::header::<ScreenshotTaken>(), id, ok, _reserved: [0; 7] }
}

#[test]
fn start_waits_until_the_core_provider_is_ready() {
    let connection = waiting();

    let state = connection.link.state();
    let set = connection.link.set_setting("overlay.radius", &hcl::Value::from(24));

    assert_eq!(state, LinkState::Waiting);
    assert_eq!(set, Err(LinkError::NotConnected));
    assert!(FakeTable::calls().is_empty(), "no table is called before ProviderReady");
    FakeTable::ready(&connection.game);
    assert_eq!(connection.link.state(), LinkState::Connected);
}

#[test]
fn connect_registers_with_the_mods_config_dir() {
    let connection = FakeTable::link_without_frame();

    assert_eq!(connection.link.state(), LinkState::Connected);
    assert_eq!(FakeTable::with(|state| state.config_dir.clone()), Some(FakeTable::CONFIG_DIR.to_owned()));
    assert_eq!(FakeTable::with(|state| state.descriptor.clone()), Some(FakeTable::DESCRIPTOR.to_owned()));
    let registers = FakeTable::calls().iter().filter(|call| **call == "register").count();
    assert_eq!(registers, 1, "one register for the five ProviderReady events");
}

#[test]
fn no_core_provider_gives_no_manager() {
    let game = FakeTable::game();
    let link =
        match ManagerLink::start_without_frame(&FakeTable::ctx_of(&game, FakeTable::DESCRIPTOR), FakeTable::DESCRIPTOR)
        {
            Ok(link) => link,
            Err(error) => panic!("start failed: {error}"),
        };

    FakeTable::mods_loaded(&game);
    FakeTable::mods_loaded(&game);

    assert_eq!(link.state(), LinkState::Waiting);
    assert_eq!(link.set_setting("a", &hcl::Value::from(1)), Err(LinkError::NoManager));
    let lines: Vec<String> = game.logs().into_iter().filter(|line| line.contains("manager not found")).collect();
    assert_eq!(lines, vec!["warn manager not found".to_owned()]);
    drop(link);
}

#[test]
fn calls_after_provider_gone_give_gone() {
    let mut connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    let handle = FakeTable::with(|state| state.handle);

    FakeTable::gone(&connection.game);
    let calls_at_gone = FakeTable::calls();
    let set = connection.link.set_setting("overlay.radius", &hcl::Value::from(24));
    let camera = connection.link.acquire(Capability::Camera);
    let input = connection.link.acquire(Capability::Input);
    let open = connection.link.panel_open();

    assert_eq!(connection.link.state(), LinkState::Gone);
    assert_eq!(set, Err(LinkError::Gone));
    assert_eq!(camera, Err(LinkError::Gone));
    assert_eq!(input, Err(LinkError::Gone));
    assert!(!open);
    assert_eq!(FakeTable::calls(), calls_at_gone, "no table call after ProviderGone");
    assert_eq!(FakeTable::with(|state| state.unregistered.clone()), vec![handle], "unregistered once, at the gone");
    assert_eq!(connection.link.events().collect::<Vec<_>>(), vec![ModEvent::ManagerGone]);
    drop(connection);
    assert_eq!(FakeTable::with(|state| state.unregistered.len()), 1, "a drop after the gone does not unregister");
}

#[test]
fn provider_gone_releases_the_input_lease_and_its_keys() {
    let connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));
    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    connection.game.frame();

    FakeTable::gone(&connection.game);
    connection.game.frame();

    assert_eq!(connection.game.injected(), vec!["key 0x57 down".to_owned(), "key 0x57 up".to_owned()]);
    assert_eq!(connection.link.input_lease_id(), None);
}

#[test]
fn provider_gone_frees_the_frame_state_after_an_ok_unregister() {
    let dropped = Arc::new(AtomicBool::new(false));
    let connection = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));

    FakeTable::gone(&connection.game);

    assert!(dropped.load(Ordering::SeqCst), "freed at the gone, before the manager is disabled");
    drop(connection);
}

#[test]
fn a_refused_unregister_keeps_the_frame_state_and_logs_an_error() {
    let dropped = Arc::new(AtomicBool::new(false));
    let FakeConnection { link, game } = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));
    FakeTable::with(|state| state.unregister_status = AmmStatus::NotPermitted);

    drop(link);

    assert!(!dropped.load(Ordering::SeqCst));
    let errors = game.logs().into_iter().filter(|line| line.starts_with("error unregister refused")).count();
    assert_eq!(errors, 1);
}

#[test]
fn dropping_the_link_unregisters_once_and_frees_the_frame_state() {
    let dropped = Arc::new(AtomicBool::new(false));
    let connection = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));
    let handle = FakeTable::with(|state| state.handle);
    assert!(!dropped.load(Ordering::SeqCst));

    drop(connection);

    assert_eq!(FakeTable::with(|state| state.unregistered.clone()), vec![handle]);
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn a_link_with_a_frame_gives_its_callbacks_at_connect_and_one_without_gives_none() {
    let with_frame = FakeTable::link(IdleHandler);
    let callbacks = FakeTable::with(|state| state.callbacks);
    let ran = FakeTable::run_frame();
    drop(with_frame);
    FakeTable::with(|state| state.calls.clear());

    let without = FakeTable::link_without_frame();

    let Some(callbacks) = callbacks else { panic!("no callbacks were set") };
    assert!(callbacks.frame.is_some() && callbacks.textures_lost.is_some());
    assert_eq!(ran, AmmStatus::Ok as i32);
    assert!(!FakeTable::calls().contains(&"set_callbacks"));
    drop(without);
}

#[test]
fn a_refused_registration_is_kept_as_the_fault() {
    FakeTable::with(|state| state.register_status = AmmStatus::DuplicateId);

    let connection = FakeTable::link_without_frame();

    assert_eq!(connection.link.state(), LinkState::Waiting);
    assert_eq!(connection.link.config::<TestConfig>(), Err(LinkError::Status(AmmStatus::DuplicateId as i32)));
    drop(connection);
    assert!(FakeTable::with(|state| state.unregistered.is_empty()));
}

#[test]
fn injected_release_becomes_fake_keys_released() {
    let mut connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));
    let Some(lease) = connection.link.input_lease_id() else { panic!("no lease") };

    let _ = connection.game.raise(&mut release_of(lease, AmmlReleaseReason::FOCUS_LOST));
    let _ = connection.game.raise(&mut release_of(lease ^ 1, AmmlReleaseReason::FOCUS_LOST));

    assert_eq!(
        connection.link.events().collect::<Vec<_>>(),
        vec![ModEvent::FakeKeysReleased { reason: ReleaseReason::FocusLost }],
        "a release of another lease is not passed on"
    );
}

#[test]
fn a_preempting_lease_suspends_the_link_lease_and_releases_its_keys() {
    let mut connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));
    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    connection.game.frame();
    let panel = LeaseRequest { mode: LeaseMode::Exclusive, blocks: InputBlocks::FULL, passthrough: &[], preempt: true };

    let preempt = match connection.game.ctx().input().acquire(panel) {
        Ok(lease) => lease,
        Err(error) => panic!("preempt refused: {error}"),
    };
    connection.game.frame();
    let suspended: Vec<ModEvent> = connection.link.events().collect();
    let refused = connection.link.send_key(KeyCode::A, true);
    drop(preempt);
    connection.game.frame();
    let resumed: Vec<ModEvent> = connection.link.events().collect();

    assert!(suspended.contains(&ModEvent::InputLeaseSuspended), "{suspended:?}");
    assert!(suspended.contains(&ModEvent::FakeKeysReleased { reason: ReleaseReason::Suspended }), "{suspended:?}");
    assert!(matches!(refused, Err(LinkError::Api(_))), "{refused:?}");
    assert!(resumed.contains(&ModEvent::InputLeaseResumed), "{resumed:?}");
    assert_eq!(connection.game.injected(), vec!["key 0x57 down".to_owned(), "key 0x57 up".to_owned()]);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));
}

#[test]
fn events_merge_manager_and_loader_events_in_arrival_order() {
    let mut connection = FakeTable::link_without_frame();
    FakeTable::push_event(ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed });
    let _ = connection.game.raise(&mut taken(4, 1));
    FakeTable::push_event(ModEvent::Keybind { action: 1, phase: KeybindPhase::Released });
    let mut written =
        ScreenshotWritten { header: FakeGame::header::<ScreenshotWritten>(), id: 4, ok: 0, _reserved: [0; 7] };
    let _ = connection.game.raise(&mut written);

    let events: Vec<ModEvent> = connection.link.events().collect();

    assert_eq!(
        events,
        vec![
            ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed },
            ModEvent::ScreenshotTaken { id: ShotId(4), ok: true },
            ModEvent::Keybind { action: 1, phase: KeybindPhase::Released },
            ModEvent::ScreenshotWritten { id: ShotId(4), ok: false },
        ]
    );
    assert_eq!(connection.link.events().count(), 0);
}

#[test]
fn lease_events_of_the_manager_reach_the_link_in_order() {
    let mut connection = FakeTable::link_without_frame();
    let sent = [
        ModEvent::Granted(Capability::WorldDraw),
        ModEvent::Suspended { capability: Capability::WorldDraw, by: ModIndex(5) },
        ModEvent::Resumed(Capability::WorldDraw),
        ModEvent::Revoked { capability: Capability::WorldDraw, by: ModIndex(6) },
        ModEvent::Disabled,
    ];
    sent.iter().for_each(|event| FakeTable::push_event(*event));

    let received: Vec<ModEvent> = connection.link.events().collect();

    assert_eq!(received, sent);
}

#[test]
fn screenshot_calls_the_overlay() {
    let mut connection = FakeTable::link_without_frame();
    let first = PathBuf::from("C:\\shots\\a.png");
    let second = PathBuf::from("C:\\shots\\b.png");

    let with_layers = connection.link.request_screenshot_with(&first, ScreenshotLayers::WithModLayers);
    let game_only = connection.link.request_screenshot_with(&second, ScreenshotLayers::GameOnly);

    let Ok(id) = with_layers else { panic!("screenshot refused: {with_layers:?}") };
    assert!(game_only.is_ok());
    assert_eq!(connection.game.screenshots(), vec![(first, ShotLayers::WithOverlays), (second, ShotLayers::GameOnly)]);
    let _ = connection.game.raise(&mut taken(id.0, 1));
    assert_eq!(connection.link.events().collect::<Vec<_>>(), vec![ModEvent::ScreenshotTaken { id, ok: true }]);
}

#[test]
fn acquire_answers_the_manager_outcome_per_capability() {
    let connection = FakeTable::link_without_frame();
    FakeTable::set_acquire_outcome(Capability::WorldDraw, AcquireOutcome::Denied { holder: ModIndex(2) });
    FakeTable::set_acquire_outcome(Capability::Camera, AcquireOutcome::Preempted);

    let world_draw = connection.link.acquire(Capability::WorldDraw);
    let camera = connection.link.acquire(Capability::Camera);
    let hud = connection.link.acquire(Capability::Hud);
    connection.link.release(Capability::Hud);

    assert_eq!(world_draw, Ok(AcquireOutcome::Denied { holder: ModIndex(2) }));
    assert_eq!(camera, Ok(AcquireOutcome::Preempted));
    assert_eq!(hud, Ok(AcquireOutcome::Granted));
    assert_eq!(
        FakeTable::with(|state| state.acquired.clone()),
        vec![Capability::WorldDraw, Capability::Camera, Capability::Hud]
    );
    assert_eq!(FakeTable::with(|state| state.released.clone()), vec![Capability::Hud]);
}

#[test]
fn input_without_an_input_claim_is_not_permitted_and_no_manager_call() {
    let connection = FakeTable::link_without_frame();
    FakeTable::with(|state| state.calls.clear());

    let input = connection.link.acquire(Capability::Input);
    let held = connection.link.held_by_other(Capability::Input);
    let holder = connection.link.holder(Capability::Input);

    assert_eq!(input, Err(LinkError::Status(AmmStatus::NotPermitted as i32)));
    assert!(!held);
    assert_eq!(holder, None);
    assert!(FakeTable::calls().is_empty(), "{:?}", FakeTable::calls());
}

#[test]
fn config_parses_the_hcl_document_into_the_mods_type_and_set_setting_sends_hcl() {
    let connection = FakeTable::link_without_frame();
    FakeTable::set_config("# kept by the manager\ncard {\n  scale = 0.75\n}\n");

    let config = connection.link.config::<TestConfig>();
    let set = connection.link.set_setting("card.scale", &hcl::Value::from(24));

    assert_eq!(config, Ok(TestConfig { card: Card { scale: 0.75 } }));
    assert_eq!(set, Ok(()));
    assert_eq!(
        FakeTable::with(|state| state.settings.clone()),
        vec![SettingCall { path: "card.scale".to_owned(), expression: "24".to_owned() }]
    );
}

#[test]
fn holder_reads_an_id_longer_than_its_first_buffer_and_locale_reads_the_code() {
    const LONG: &str = "a-mod-id-that-is-longer-than-sixty-four-bytes-so-the-first-buffer-is-too-small";
    let connection = FakeTable::link_without_frame();
    let nobody = connection.link.holder(Capability::Camera);
    FakeTable::with(|state| {
        state.holder_id = Some(LONG);
        state.locale = "en_XA";
    });

    assert_eq!(nobody, None);
    assert_eq!(connection.link.holder(Capability::Overlay).as_deref(), Some(LONG));
    assert_eq!(connection.link.locale().as_deref(), Some("en_XA"));
}

#[test]
fn the_link_gives_back_the_localizer_it_was_set() {
    let connection = FakeTable::link(IdleHandler);
    let set = Arc::new(amml_i18n::localize::Localizer::english());

    connection.link.set_localizer(Arc::clone(&set));

    assert!(Arc::ptr_eq(&connection.link.localizer(), &set));
}

#[test]
fn a_descriptor_that_does_not_parse_is_json_and_frees_the_frame_state() {
    let game = FakeTable::game();
    let dropped = Arc::new(AtomicBool::new(false));

    let result = ManagerLink::start(&game.ctx(), "claim {", Some(DropFlagHandler::new(Arc::clone(&dropped))));

    assert!(matches!(result, Err(LinkError::Json { .. })));
    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn tracing_events_reach_the_mod_log() {
    let connection = FakeTable::link_without_frame();

    tracing::info!(chunk = 12, "from a tracing library");

    assert!(connection.game.logs().contains(&"info from a tracing library chunk=12".to_owned()));
}

#[test]
fn the_crate_has_no_module_lookup() {
    let mut pending = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
    let mut hits = Vec::new();
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { panic!("cannot read {}", dir.display()) };
        for path in entries.flatten().map(|entry| entry.path()) {
            if path.is_dir() {
                pending.push(path);
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { panic!("cannot read {}", path.display()) };
            for word in ["GetModuleHandle", "GetProcAddress", "amml_sdk", "amm_get_api"] {
                if text.contains(word) {
                    hits.push(format!("{} {word}", path.display()));
                }
            }
        }
    }

    assert_eq!(hits, Vec::<String>::new());
}

#[test]
fn a_real_up_that_ends_a_held_key_drops_only_that_hold_and_a_new_down_presses_again() {
    let mut connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    assert_eq!(connection.link.acquire(Capability::Input), Ok(AcquireOutcome::Granted));
    assert_eq!(connection.link.send_key(KeyCode::LSHIFT, true), Ok(()));
    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    connection.game.frame();

    connection.game.real_key(KeyCode::W.0, false);
    connection.game.frame();
    assert_eq!(connection.link.send_key(KeyCode::W, true), Ok(()));
    connection.game.frame();

    assert_eq!(
        connection.game.injected(),
        vec!["key 0xA0 down".to_owned(), "key 0x57 down".to_owned(), "key 0x57 down".to_owned()],
        "W is pressed again and Shift stays down"
    );
    let events: Vec<ModEvent> = connection.link.events().collect();
    assert_eq!(events, vec![ModEvent::FakeKeysReleased { reason: ReleaseReason::Reset }]);
}

#[test]
fn a_descriptor_for_another_mod_id_is_refused() {
    let game = FakeTable::game();

    let result = ManagerLink::start_without_frame(&game.ctx_named("@other/mod"), FakeTable::DESCRIPTOR);

    assert_eq!(
        result.err(),
        Some(LinkError::IdMismatch { descriptor: "@test/alaydriem-test".to_owned(), module: "@other/mod".to_owned() })
    );
    assert!(FakeTable::calls().is_empty(), "no register for a refused link");
}

#[test]
fn input_on_a_loader_without_the_lease_area_is_not_available() {
    let connection = FakeTable::link_without_frame_with(INPUT_DESCRIPTOR);
    connection.game.withdraw_area(amml_api::Area::INPUT_LEASE);

    assert_eq!(connection.link.acquire(Capability::Input), Err(LinkError::Api(amml_api::ApiError::NotAvailable)));
}

#[test]
fn unread_events_are_bounded_and_the_oldest_go_first() {
    let mut connection = FakeTable::link_without_frame();
    for id in 0..300u64 {
        let _ = connection.game.raise(&mut taken(id, 1));
    }

    let events: Vec<ModEvent> = connection.link.events().collect();

    assert_eq!(events.len(), 256);
    assert_eq!(events.first(), Some(&ModEvent::ScreenshotTaken { id: ShotId(44), ok: true }));
    assert_eq!(events.last(), Some(&ModEvent::ScreenshotTaken { id: ShotId(299), ok: true }));
}
