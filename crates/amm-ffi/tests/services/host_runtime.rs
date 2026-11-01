use amm_abi::types::AmmCallbacks;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::input::InputEvent;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::toast::ToastKind;
use amm_core::structs::write::SaveStatus;
use amm_ffi::services::{HostRuntime, LogBridge};
use amm_ffi::types::AmmHostFrameInput;
use amm_ui::model::PanelAction;

use crate::support::Descriptors;

const NO_CALLBACKS: AmmCallbacks = AmmCallbacks {
    size: size_of::<AmmCallbacks>() as u32,
    user: std::ptr::null_mut(),
    frame: None,
    textures_lost: None,
};
const SCHEMA: &str = "section \"keys\" {\n  title = \"Keys\"\n  field \"keys.toggle\" {\n    label = \"Toggle\"\n    \
                      kind = \"chord\"\n    default = \"Ctrl+L\"\n    apply = \"Live\"\n  }\n}\n";

#[test]
fn a_full_io_queue_shows_save_failed_and_keeps_the_write_to_try_again() {
    let dir = tempfile::tempdir().expect("temp dir");
    let manager_dir = dir.path().join("manager");
    std::fs::create_dir_all(&manager_dir).expect("manager folder");
    std::fs::write(manager_dir.join("config.hcl"), "budgets {\n  io_queue = 1\n}\n").expect("manager config");
    let mut runtime = HostRuntime::new(manager_dir, dir.path().join("game"), "test".into()).expect("runtime starts");
    let mut indices = Vec::new();
    for id in ["saver", "saver-two"] {
        let mod_dir = dir.path().join("mods").join(id).join("config");
        let descriptor = Descriptors::with_claims(id, &[("Overlay", "Shared")], SCHEMA);
        let handle = runtime.register(descriptor.as_bytes(), &mod_dir, NO_CALLBACKS).expect("registered");
        indices.push(ModIndex(handle.slot as u8));
    }
    runtime.tick(1000);
    runtime.route(InputEvent::PointerMove { x: 0.0, y: 0.0 }, 5000);

    let edits = indices.iter().map(|&mod_index| PanelAction::Edit {
        mod_index,
        path: "keys.toggle".into(),
        value: hcl::Value::from("Ctrl+K"),
    });
    runtime.apply_panel_actions(&edits.collect::<Vec<_>>());
    runtime.tick(5600);

    let toasts = runtime.manager().toasts();
    assert!(toasts.visible(0, u64::MAX).any(|toast| toast.kind == ToastKind::SaveFailed && toast.created_ms == 5600));
    assert_eq!(runtime.manager().save_status(5600), SaveStatus::Failed);
    runtime.shutdown();
}

#[test]
fn a_mod_config_that_cannot_be_kept_as_invalid_is_never_overwritten() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime =
        HostRuntime::new(dir.path().join("manager"), dir.path().join("game"), "test".into()).expect("runtime starts");
    let mod_dir = dir.path().join("mods").join("keeper").join("config");
    std::fs::create_dir_all(mod_dir.join("config.hcl.invalid")).expect("blocked copy");
    std::fs::write(mod_dir.join("config.hcl"), "keys { toggle = = }\n").expect("hand edit");
    let descriptor = Descriptors::with_claims("keeper", &[("Overlay", "Shared")], SCHEMA);

    let handle = runtime.register(descriptor.as_bytes(), &mod_dir, NO_CALLBACKS).expect("registered");
    let document = runtime.manager().document(ModIndex(handle.slot as u8)).map(|document| document.text());
    runtime.tick(u64::MAX / 2);
    runtime.shutdown();

    assert!(document.is_some_and(|text| text.contains("Ctrl+L")));
    assert_eq!(std::fs::read_to_string(mod_dir.join("config.hcl")).expect("kept"), "keys { toggle = = }\n");
}

#[test]
fn a_config_that_could_not_be_read_is_never_written() {
    let dir = tempfile::tempdir().expect("temp dir");
    let game = dir.path().join("game");
    let mut runtime =
        HostRuntime::new(dir.path().join("manager"), game.clone(), "test".into()).expect("runtime starts");
    let mod_dir = dir.path().join("mods").join("keeper").join("config");
    std::fs::create_dir_all(mod_dir.join("config.hcl.invalid")).expect("blocked copy");
    std::fs::write(mod_dir.join("config.hcl"), "keys { toggle = = }\n").expect("hand edit");
    let descriptor = Descriptors::with_claims("keeper", &[("Overlay", "Shared")], SCHEMA);
    let handle = runtime.register(descriptor.as_bytes(), &mod_dir, NO_CALLBACKS).expect("registered");
    let index = ModIndex(handle.slot as u8);
    runtime.tick(1000);
    assert!(runtime.manager().toasts().visible(0, u64::MAX).next().is_none());

    let in_play: Vec<_> = (0..5).map(|_| runtime.set_setting(handle, "keys.toggle", "\"Ctrl+K\"")).collect();
    let edit = PanelAction::Edit { mod_index: index, path: "keys.toggle".into(), value: hcl::Value::from("Ctrl+J") };
    runtime.apply_panel_actions(&[edit]);
    runtime.tick(1700);
    let header = runtime.manager().save_status(1700);
    let in_window = save_failed_toasts(&runtime);
    runtime.tick(5000);
    let later = runtime.set_setting(handle, "keys.toggle", "\"Ctrl+M\"");
    let after_window = save_failed_toasts(&runtime);
    runtime.shutdown();

    assert!(in_play.iter().all(Result::is_ok) && later.is_ok());
    assert_eq!(header, SaveStatus::Failed);
    assert_eq!(in_window, 1);
    assert_eq!(after_window, 2);
    assert_eq!(manager_log(&game).matches("config write refused").count(), 2);
    assert_eq!(std::fs::read_to_string(mod_dir.join("config.hcl")).expect("kept"), "keys { toggle = = }\n");
}

#[test]
fn a_mod_registered_without_a_frame_callback_draws_no_layer_and_counts_no_failure() {
    let dir = tempfile::tempdir().expect("temp dir");
    let mut runtime =
        HostRuntime::new(dir.path().join("manager"), dir.path().join("game"), "test".into()).expect("runtime starts");
    let mod_dir = dir.path().join("mods").join("quiet").join("config");
    let descriptor = Descriptors::with_claims("quiet", &[("Hud", "Shared")], "");
    let handle = runtime.register(descriptor.as_bytes(), &mod_dir, NO_CALLBACKS).expect("registered");
    runtime.acquire(handle, Capability::Hud).expect("acquired");

    let input = AmmHostFrameInput {
        width: 1920,
        height: 1080,
        pixels_per_point: 1.0,
        time_secs: 0.0,
        now_ms: 0,
        menu_screen: 0,
        format: 28,
        client_width: 1920,
        client_height: 1080,
    };
    let layers: Vec<usize> = (0..40).map(|_| runtime.frame(&input).layer_count as usize).collect();
    let failed = runtime.manager().registry().get(ModIndex(handle.slot as u8)).is_some_and(|m| m.failed);
    runtime.shutdown();

    assert!(layers.iter().all(|&count| count == 0));
    assert!(!failed);
}

fn save_failed_toasts(runtime: &HostRuntime) -> usize {
    runtime.manager().toasts().visible(0, u64::MAX).filter(|toast| toast.kind == ToastKind::SaveFailed).count()
}

fn manager_log(game: &std::path::Path) -> String {
    let dir = game.join("logs").join("amm").join("@amml").join(LogBridge::MANAGER_NAME);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return String::new();
    };
    entries.flatten().map(|entry| std::fs::read_to_string(entry.path()).unwrap_or_default()).collect()
}

#[test]
fn creating_a_host_never_replaces_an_open_log_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let game = dir.path().join("game");

    let runtime = HostRuntime::new(dir.path().join("manager"), game.clone(), "test".into()).expect("runtime starts");
    let replaced = LogBridge::current().map(LogBridge::replaced_sinks);
    runtime.shutdown();

    assert_eq!(replaced, Some(0));
    assert!(manager_log(&game).contains("mod manager started"));
}

#[test]
fn a_host_made_again_after_shutdown_logs_to_the_same_manager_file() {
    let dir = tempfile::tempdir().expect("temp dir");
    let game = dir.path().join("game");

    for _ in 0..2 {
        let runtime =
            HostRuntime::new(dir.path().join("manager"), game.clone(), "test".into()).expect("runtime starts");
        runtime.shutdown();
    }

    assert_eq!(LogBridge::current().map(LogBridge::replaced_sinks), Some(0));
    let text = manager_log(&game);
    assert_eq!(text.matches("mod manager started").count(), 2);
    assert_eq!(text.matches("mod manager stopped").count(), 2);
}
