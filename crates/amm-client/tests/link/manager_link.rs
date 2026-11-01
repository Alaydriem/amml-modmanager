use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use amm_abi::types::{AmmEvent, AmmLogLevel, AmmRenderSize, AmmRenderSizeState, AmmStatus};
use amm_client::link::ManagerLink;
use amm_core::errors::LinkError;
use amm_core::services::icon::IconDecoder;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::mesh::GamePicture;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::render::{RenderSize, RenderSizeState};
use serde::Deserialize;

use crate::support::{DropFlagHandler, FakeTable, IdleHandler};

const DESCRIPTOR: &str = "namespace = \"test\"\nname = \"alaydriem-refused\"\n";

#[derive(Debug, Deserialize, PartialEq)]
struct Card {
    scale: f32,
}

#[derive(Debug, Deserialize, PartialEq)]
struct Keys {
    take_camera: String,
}

#[derive(Debug, Deserialize, PartialEq)]
struct TestConfig {
    card: Card,
    keys: Keys,
}

#[test]
fn dropping_the_link_unregisters_once() {
    let link = FakeTable::link(IdleHandler);
    let handle = FakeTable::with(|state| state.handle);
    assert!(FakeTable::with(|state| state.unregistered.is_empty()));

    drop(link);

    assert_eq!(FakeTable::with(|state| state.unregistered.clone()), vec![handle]);
}

#[test]
fn register_gives_the_descriptor_text_and_a_refusal_is_the_status() {
    FakeTable::with(|state| state.register_status = AmmStatus::DuplicateId);

    let result = ManagerLink::new(FakeTable::table(), DESCRIPTOR, Path::new(FakeTable::CONFIG_DIR), IdleHandler);

    assert_eq!(result.err(), Some(LinkError::Status(AmmStatus::DuplicateId as i32)));
    assert_eq!(FakeTable::with(|state| state.descriptor.clone()), Some(DESCRIPTOR.to_owned()));
    assert_eq!(FakeTable::with(|state| state.config_dir.clone()), Some(FakeTable::CONFIG_DIR.to_owned()));
    assert!(FakeTable::with(|state| state.unregistered.is_empty()));
}

#[test]
fn events_come_back_as_core_events_in_order() {
    let mut link = FakeTable::link(IdleHandler);
    let sent = vec![
        ModEvent::Granted(Capability::Camera),
        ModEvent::Keybind { action: 2, phase: KeybindPhase::Pressed },
        ModEvent::Revoked { capability: Capability::Input, by: ModIndex(4) },
        ModEvent::Disabled,
    ];
    FakeTable::with(|state| state.events.extend(sent.iter().map(|event| AmmEvent::from(*event))));

    let received: Vec<ModEvent> = link.events().collect();

    assert_eq!(received, sent);
    assert_eq!(link.events().count(), 0);
}

#[test]
fn config_parses_the_hcl_document_into_the_mods_type() {
    let link = FakeTable::link(IdleHandler);
    FakeTable::set_config(
        "# kept by the manager\ncard {\n  scale = 0.75\n}\nkeys {\n  take_camera = \"Ctrl+Shift+C\"\n}\n",
    );

    let config: TestConfig = match link.config() {
        Ok(config) => config,
        Err(error) => panic!("config did not parse: {error}"),
    };

    assert_eq!(
        config,
        TestConfig { card: Card { scale: 0.75 }, keys: Keys { take_camera: "Ctrl+Shift+C".to_owned() } }
    );
}

#[test]
fn config_that_does_not_fit_the_mods_type_is_an_error() {
    let link = FakeTable::link(IdleHandler);
    FakeTable::set_config("card {\n  scale = \"large\"\n}\n");

    let result = link.config::<TestConfig>();

    assert!(matches!(result, Err(LinkError::Json { .. })));
}

#[test]
fn tracing_events_reach_the_table_through_the_bridge() {
    let link = FakeTable::link(IdleHandler);

    tracing::info!(chunk = 12, "from a tracing library");

    let messages: Vec<String> = FakeTable::with(|state| state.logs.iter().map(|call| call.message.clone()).collect());
    assert_eq!(messages, vec!["from a tracing library".to_owned()]);
    drop(link);
}

#[test]
fn logging_after_the_link_is_dropped_does_not_reach_the_table() {
    let link = FakeTable::link(IdleHandler);
    curia::info!("while linked", { n: 1 });
    drop(link);

    curia::info!("after unlink", { n: 2 });

    let messages: Vec<String> = FakeTable::with(|state| state.logs.iter().map(|call| call.message.clone()).collect());
    assert_eq!(messages, vec!["while linked".to_owned()]);
}

#[test]
fn the_link_sink_logs_through_the_links_handle_until_the_link_drops() {
    let link = FakeTable::link(IdleHandler);
    let sink = link.sink();
    let event = curia::LogEvent {
        level: curia::Level::Info,
        target: "demo".to_owned(),
        message: "hello".to_owned(),
        fields: curia::Fields::new(),
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    };

    curia::Sink::emit(&sink, &event);
    drop(link);
    curia::Sink::emit(&sink, &event);

    assert_eq!(
        FakeTable::with(|state| state.logs.iter().map(|call| call.message.clone()).collect::<Vec<_>>()),
        vec!["hello"]
    );
}

#[test]
fn an_unregister_that_returns_ok_frees_the_frame_state() {
    let dropped = Arc::new(AtomicBool::new(false));
    let link = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));

    drop(link);

    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn a_refused_unregister_keeps_the_frame_state_and_logs_an_error() {
    let dropped = Arc::new(AtomicBool::new(false));
    let link = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));
    FakeTable::with(|state| state.unregister_status = AmmStatus::NotPermitted);

    drop(link);

    assert!(!dropped.load(Ordering::SeqCst));
    let errors =
        FakeTable::with(|state| state.logs.iter().filter(|call| call.level == AmmLogLevel::Error as u8).count());
    assert_eq!(errors, 1);
}

#[test]
fn holder_copies_the_id_and_none_when_no_mod_holds_it() {
    let link = FakeTable::link(IdleHandler);
    let nobody = link.holder(Capability::Camera);
    FakeTable::with(|state| state.holder_id = Some("alaydriem-freecam"));

    let holder = link.holder(Capability::Camera);

    assert_eq!(nobody, None);
    assert_eq!(holder.as_deref(), Some("alaydriem-freecam"));
}

#[test]
fn holder_reads_an_id_longer_than_its_first_buffer() {
    const LONG: &str = "a-mod-id-that-is-longer-than-sixty-four-bytes-so-the-first-buffer-is-too-small";
    let link = FakeTable::link(IdleHandler);
    FakeTable::with(|state| state.holder_id = Some(LONG));

    let holder = link.holder(Capability::Overlay);

    assert_eq!(holder.as_deref(), Some(LONG));
}

#[test]
fn link_reads_the_locale() {
    let link = FakeTable::link(IdleHandler);
    let english = link.locale();
    FakeTable::with(|state| state.locale = "en_XA");

    let pseudo = link.locale();

    assert_eq!(english.as_deref(), Some(""));
    assert_eq!(pseudo.as_deref(), Some("en_XA"));
}

#[test]
fn the_link_gives_back_the_localizer_it_was_set() {
    let link = FakeTable::link(IdleHandler);
    let set = Arc::new(amml_i18n::localize::Localizer::english());

    link.set_localizer(Arc::clone(&set));

    assert!(Arc::ptr_eq(&link.localizer(), &set));
}

#[test]
fn a_link_without_a_frame_registers_with_no_callbacks_and_unregisters_on_drop() {
    let link = FakeTable::link_without_frame();
    let handle = FakeTable::with(|state| state.handle);
    let callbacks = FakeTable::with(|state| state.callbacks);

    drop(link);

    let Some(callbacks) = callbacks else { panic!("nothing registered") };
    assert!(callbacks.frame.is_none());
    assert!(callbacks.textures_lost.is_none());
    assert!(callbacks.user.is_null());
    assert!(FakeTable::with(|state| state.registered_without_frame()));
    assert_eq!(FakeTable::with(|state| state.descriptor.clone()), Some(FakeTable::DESCRIPTOR.to_owned()));
    assert_eq!(FakeTable::with(|state| state.unregistered.clone()), vec![handle]);
}

#[test]
fn a_link_without_a_frame_gets_keybind_and_settings_events() {
    let mut link = FakeTable::link_without_frame();
    let sent = [
        ModEvent::Enabled,
        ModEvent::Keybind { action: 1, phase: KeybindPhase::Pressed },
        ModEvent::SettingsChanged { paths: 2 },
        ModEvent::Keybind { action: 1, phase: KeybindPhase::Released },
    ];
    sent.iter().for_each(|event| FakeTable::push_event(*event));

    let received: Vec<ModEvent> = link.events().collect();

    assert_eq!(received, sent);
}

#[test]
fn a_link_without_a_frame_forwards_its_logs_until_it_drops() {
    let link = FakeTable::link_without_frame();
    curia::info!("while linked", { n: 1 });
    drop(link);

    curia::info!("after unlink", { n: 2 });

    let messages: Vec<String> = FakeTable::with(|state| state.logs.iter().map(|call| call.message.clone()).collect());
    assert_eq!(messages, vec!["while linked".to_owned()]);
}

#[test]
fn a_link_without_a_frame_and_a_refused_unregister_logs_one_error() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.unregister_status = AmmStatus::NotPermitted);

    drop(link);

    let errors =
        FakeTable::with(|state| state.logs.iter().filter(|call| call.level == AmmLogLevel::Error as u8).count());
    assert_eq!(errors, 1);
}

#[test]
fn a_refused_register_without_a_frame_is_the_status() {
    FakeTable::with(|state| state.register_status = AmmStatus::DuplicateId);

    let result = ManagerLink::without_frame(FakeTable::table(), DESCRIPTOR, Path::new(FakeTable::CONFIG_DIR));

    assert_eq!(result.err(), Some(LinkError::Status(AmmStatus::DuplicateId as i32)));
    assert!(FakeTable::with(|state| state.unregistered.is_empty()));
}

#[test]
fn a_link_made_with_a_frame_handler_frees_its_frame_state_on_drop() {
    let dropped = Arc::new(AtomicBool::new(false));
    let link = FakeTable::link(DropFlagHandler::new(Arc::clone(&dropped)));
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(FakeTable::with(|state| !state.registered_without_frame()));

    drop(link);

    assert!(dropped.load(Ordering::SeqCst));
}

#[test]
fn set_icon_passes_the_bytes() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.set_icon(&[1, 2, 3, 4, 5]), Ok(()));

    assert_eq!(FakeTable::with(|state| state.icon_calls.clone()), vec![5]);
}

#[test]
fn set_icon_maps_status() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.icon_status = AmmStatus::InvalidArgument);

    assert_eq!(link.set_icon(&[1, 2, 3]), Err(LinkError::Status(AmmStatus::InvalidArgument as i32)));
}

#[test]
fn old_manager_gives_unsupported() {
    let mut api = FakeTable::api();
    api.set_icon = None;
    let link = FakeTable::link_without_frame_on(api);

    assert_eq!(link.set_icon(&[1, 2, 3]), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert!(FakeTable::with(|state| state.icon_calls.is_empty()));
}

#[test]
fn set_icon_file_reads_and_sends() {
    let link = FakeTable::link_without_frame();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("icon.png");
    std::fs::write(&path, vec![7u8; 321]).expect("write icon");

    assert_eq!(link.set_icon_file(&path), Ok(()));

    assert_eq!(FakeTable::with(|state| state.icon_calls.clone()), vec![321]);
}

#[test]
fn oversized_file_is_refused_without_a_call() {
    let link = FakeTable::link_without_frame();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("big.png");
    std::fs::write(&path, vec![0u8; IconDecoder::MAX_BYTES + 1]).expect("write icon");

    let result = link.set_icon_file(&path);

    assert!(matches!(result, Err(LinkError::IconFile { .. })), "got {result:?}");
    assert!(FakeTable::with(|state| state.icon_calls.is_empty()));
}

#[test]
fn empty_icon_file_is_refused_without_a_call() {
    let link = FakeTable::link_without_frame();
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("empty.png");
    std::fs::write(&path, b"").expect("write icon");

    let result = link.set_icon_file(&path);

    assert!(matches!(result, Err(LinkError::IconFile { .. })), "got {result:?}");
    assert!(FakeTable::with(|state| state.icon_calls.is_empty()));
}

#[test]
fn missing_file_is_icon_file_error() {
    let link = FakeTable::link_without_frame();
    let dir = tempfile::tempdir().expect("temp dir");

    let result = link.set_icon_file(&dir.path().join("absent.png"));

    assert!(matches!(result, Err(LinkError::IconFile { .. })), "got {result:?}");
    assert!(FakeTable::with(|state| state.icon_calls.is_empty()));
}

#[test]
fn game_picture_gives_the_size_the_manager_reports() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.game_picture = (AmmStatus::Ok, 1920, 1080));

    assert_eq!(link.game_picture(), Ok(GamePicture::new(1920, 1080)));
}

#[test]
fn game_picture_maps_a_refusal() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.game_picture = (AmmStatus::NotPermitted, 0, 0));

    assert_eq!(link.game_picture(), Err(LinkError::Status(AmmStatus::NotPermitted as i32)));
}

#[test]
fn game_picture_on_an_old_manager_is_unsupported() {
    let mut api = FakeTable::api();
    api.game_picture = None;
    let link = FakeTable::link_without_frame_on(api);

    assert_eq!(link.game_picture(), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
}

#[test]
fn set_game_viewport_sends_the_rect_and_none_clears() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.set_game_viewport(Some([1.0, 2.0, 3.0, 4.0])), Ok(()));
    assert_eq!(link.set_game_viewport(None), Ok(()));

    assert_eq!(FakeTable::with(|state| state.game_viewports.clone()), vec![Some([1.0, 2.0, 3.0, 4.0]), None]);
}

#[test]
fn set_game_viewport_on_an_old_manager_is_unsupported() {
    let mut api = FakeTable::api();
    api.set_game_viewport = None;
    let link = FakeTable::link_without_frame_on(api);

    assert_eq!(link.set_game_viewport(None), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
}

#[test]
fn set_render_size_sends_the_size_and_none_clears() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.set_render_size(Some(RenderSize::new(3840, 2160))), Ok(()));
    assert_eq!(link.set_render_size(None), Ok(()));

    assert_eq!(
        FakeTable::with(|state| state.render_sizes.clone()),
        vec![Some(AmmRenderSize { width: 3840, height: 2160 }), None]
    );
}

#[test]
fn set_render_size_maps_a_refusal() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.render_size_status = AmmStatus::NotPermitted);

    assert_eq!(
        link.set_render_size(Some(RenderSize::new(3840, 2160))),
        Err(LinkError::Status(AmmStatus::NotPermitted as i32))
    );
}

#[test]
fn render_size_reads_a_zero_wanted_size_as_none() {
    let link = FakeTable::link_without_frame();
    let actual = AmmRenderSize { width: 1600, height: 900 };
    let state = AmmRenderSizeState {
        size: size_of::<AmmRenderSizeState>() as u32,
        wanted: AmmRenderSize { width: 0, height: 0 },
        actual,
        format: 28,
    };
    FakeTable::with(|fake| fake.render_size_state = (AmmStatus::Ok, state));

    assert_eq!(
        link.render_size(),
        Ok(RenderSizeState { wanted: None, actual: RenderSize::new(1600, 900), format: 28 })
    );
}

#[test]
fn render_size_reads_the_wanted_and_the_actual_size() {
    let link = FakeTable::link_without_frame();
    let size = AmmRenderSize { width: 3840, height: 2160 };
    let state =
        AmmRenderSizeState { size: size_of::<AmmRenderSizeState>() as u32, wanted: size, actual: size, format: 28 };
    FakeTable::with(|fake| fake.render_size_state = (AmmStatus::Ok, state));

    let read = link.render_size();

    assert_eq!(
        read.map(|state| (state.wanted, state.actual)),
        Ok((Some(RenderSize::new(3840, 2160)), RenderSize::new(3840, 2160)))
    );
}

#[test]
fn render_size_on_an_old_manager_is_unsupported() {
    let mut api = FakeTable::api();
    api.set_render_size = None;
    api.render_size = None;
    let link = FakeTable::link_without_frame_on(api);

    assert_eq!(link.set_render_size(None), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert_eq!(link.render_size(), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
}

#[test]
fn present_count_reads_the_table() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.present_count = (AmmStatus::Ok, 4_000_000_001));

    let count = link.present_count();
    FakeTable::with(|state| state.present_count = (AmmStatus::UnknownHandle, 0));
    let refused = link.present_count();

    assert_eq!(count, Ok(4_000_000_001));
    assert_eq!(refused, Err(LinkError::from(AmmStatus::UnknownHandle)));
}

#[test]
fn watch_keys_passes_slice() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.watch_keys(&[0x57, 0x41, 0x53]), Ok(()));
    assert_eq!(FakeTable::with(|state| state.watched()), vec![0x57, 0x41, 0x53]);

    assert_eq!(link.watch_keys(&[]), Ok(()));
    assert!(FakeTable::with(|state| state.watched()).is_empty());
}

#[test]
fn send_key_passes_vk_and_down() {
    let link = FakeTable::link_without_frame();

    assert_eq!(link.send_key(0x10, true), Ok(()));
    assert_eq!(link.send_key(0x10, false), Ok(()));

    assert_eq!(FakeTable::with(|state| state.sent_keys()), vec![(0x10, true), (0x10, false)]);
}

#[test]
fn send_key_on_minor_0_table_is_unsupported() {
    let link = FakeTable::link_without_frame_on(FakeTable::api_minor_0());

    assert_eq!(link.send_key(0x10, true), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert_eq!(link.watch_keys(&[0x57]), Err(LinkError::Status(AmmStatus::Unsupported as i32)));
    assert!(FakeTable::with(|state| state.sent_keys().is_empty() && state.watched().is_empty()));
}

#[test]
fn send_key_maps_not_permitted() {
    let link = FakeTable::link_without_frame();
    FakeTable::with(|state| state.send_key_status = AmmStatus::NotPermitted);

    assert_eq!(link.send_key(0x10, true), Err(LinkError::Status(AmmStatus::NotPermitted as i32)));
}
