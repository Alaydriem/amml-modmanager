use std::path::{Path, PathBuf};

use amm_core::config::{ExclusiveResolution, ManagerConfig, PairKey, SuspensionResolution};
use amm_core::errors::ManagerError;
use amm_core::services::manager::ManagerService;
use amm_core::services::settings::ManagerConfigFile;
use amm_core::structs::descriptor::{Capability, Chord};
use amm_core::structs::event::{KeybindPhase, ModEvent};
use amm_core::structs::frame::{CursorShape, FrameReplyData, LayerKind, MenuScreen};
use amm_core::structs::input::{FakeKeyOp, InputEvent, Modifiers};
use amm_core::structs::lease::AcquireOutcome;
use amm_core::structs::registry::ModIndex;
use amm_core::structs::settings::{ConfigWarning, HclDocument};
use amm_core::structs::snapshot::LayerEntry;
use amm_core::structs::toast::ToastKind;
use amm_core::structs::write::{SaveStatus, WriteRequest};

const FREECAM: &str = include_str!("../../../fixtures/four_mods/freecam.hcl");
const DEJAVU: &str = include_str!("../../../fixtures/four_mods/dejavu.hcl");
const MINIMAP: &str = include_str!("../../../fixtures/four_mods/minimap.hcl");
const LEVELS: &str = include_str!("../../../fixtures/four_mods/levels.hcl");

const NO_MODS: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };
const CTRL: Modifiers = Modifiers { ctrl: true, shift: false, alt: false };

const KEY_G: u32 = 0x47;
const KEY_H: u32 = 0x48;
const KEY_J: u32 = 0x4A;
const KEY_O: u32 = 0x4F;
const KEY_P: u32 = 0x50;
const KEY_W: u32 = 0x57;
const KEY_F9: u32 = 0x78;

const HUD: u8 = 1;
const OVERLAY: u8 = 2;

fn manager() -> ManagerService {
    ManagerService::new(ManagerConfig::default(), None)
}

fn dir(id: &str) -> PathBuf {
    Path::new("mods").join(id)
}

fn register(manager: &mut ManagerService, hcl: &str, id: &str) -> ModIndex {
    manager.register(hcl, None, &dir(id)).unwrap_or_else(|error| panic!("{id} does not register: {error}")).0
}

// A descriptor with the given claims (capability, mode) and extra HCL after them.
fn descriptor(id: &str, claims: &[(&str, &str)], extra: &str) -> String {
    let mut text = format!(
        "namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n"
    );
    for (capability, mode) in claims {
        text.push_str(&format!("claim \"{capability}\" {{\n  mode = \"{mode}\"\n  reason = \"r\"\n}}\n"));
    }
    text.push_str(extra);
    text
}

fn events(manager: &mut ManagerService, index: ModIndex) -> Vec<ModEvent> {
    let mut out = [ModEvent::Enabled; 64];
    let count = manager.poll_events(index, &mut out);
    out[..count].to_vec()
}

fn key(vk: u32, down: bool, mods: Modifiers) -> InputEvent {
    InputEvent::Key { vk, down, repeat: false, mods }
}

fn toasts(manager: &ManagerService) -> Vec<ToastKind> {
    manager.toasts().visible(0, u64::MAX).map(|toast| toast.kind).collect()
}

fn action(manager: &ManagerService, index: ModIndex, name: &str) -> u16 {
    let binds =
        manager.registry().get(index).map(|registered| registered.descriptor.keybinds.as_slice()).unwrap_or(&[]);
    let position = binds.iter().position(|bind| bind.action == name);
    position.unwrap_or_else(|| panic!("no keybind {name}")) as u16
}

fn layers(manager: &ManagerService) -> Vec<LayerEntry> {
    manager.snapshot().layers.clone()
}

#[test]
fn register_checks_the_document_and_returns_warnings() {
    let mut manager = manager();
    let document = HclDocument::new("# hand note\nkeys {\n  toggle = 5\n}\n").expect("parses");

    let (_, checked) = manager.register(LEVELS, Some(document), &dir("levels")).expect("registers");
    manager.register(MINIMAP, None, &dir("minimap")).expect("registers");

    assert_eq!(checked.warnings, vec![ConfigWarning::Defaulted { path: "keys.toggle".into() }]);
    let writes: Vec<WriteRequest> = manager.due_writes(0).collect();
    assert_eq!(writes.len(), 2);
    let levels = writes.iter().find(|write| write.path == dir("levels").join("config.hcl")).expect("levels write");
    assert!(levels.text.contains("# hand note"));
    assert!(levels.text.contains("\"Ctrl+L\""));
    let minimap = writes.iter().find(|write| write.path == dir("minimap").join("config.hcl")).expect("minimap write");
    assert!(minimap.text.contains("toggle_world_map = \"M\""));
}

#[test]
fn registering_a_valid_file_queues_no_write() {
    let mut manager = manager();
    let document = HclDocument::new("keys {\n  toggle = \"Ctrl+L\"\n}\n").expect("parses");

    let (_, checked) = manager.register(LEVELS, Some(document), &dir("levels")).expect("registers");

    assert!(checked.warnings.is_empty());
    assert_eq!(manager.due_writes(0).count(), 0);
}

#[test]
fn register_refuses_a_bad_descriptor_and_a_duplicate_id() {
    let mut manager = manager();
    register(&mut manager, LEVELS, "levels");

    assert!(matches!(manager.register("id = ", None, &dir("x")), Err(ManagerError::Descriptor(_))));
    assert!(matches!(manager.register(LEVELS, None, &dir("again")), Err(ManagerError::Registry(_))));
}

#[test]
fn a_mod_disabled_in_the_config_gets_disabled_first() {
    let mut config = ManagerConfig::default();
    config.enabled.insert("@alaydriem/freecam".into(), false);
    let mut manager = ManagerService::new(config, None);

    let freecam = register(&mut manager, FREECAM, "freecam");

    assert_eq!(events(&mut manager, freecam), vec![ModEvent::Disabled]);
    assert!(matches!(manager.acquire(freecam, Capability::Overlay, 0), Err(ManagerError::NotPermitted)));
}

#[test]
fn disabling_a_mod_releases_its_leases_and_drops_its_layer() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    assert_eq!(manager.acquire(freecam, Capability::Overlay, 0).expect("known"), AcquireOutcome::Granted);
    assert_eq!(manager.acquire(freecam, Capability::Camera, 0).expect("known"), AcquireOutcome::Granted);
    assert_eq!(layers(&manager).len(), 1);
    events(&mut manager, freecam);

    manager.set_enabled(freecam, false);

    assert!(layers(&manager).is_empty());
    assert!(!manager.arbiter().holds(freecam, Capability::Overlay));
    assert!(!manager.arbiter().holds(freecam, Capability::Camera));
    assert_eq!(events(&mut manager, freecam), vec![ModEvent::Disabled]);
    assert!(matches!(manager.acquire(freecam, Capability::Overlay, 0), Err(ManagerError::NotPermitted)));

    manager.set_enabled(freecam, true);

    assert_eq!(events(&mut manager, freecam), vec![ModEvent::Enabled]);
    assert_eq!(manager.acquire(freecam, Capability::Overlay, 0).expect("known"), AcquireOutcome::Granted);
}

#[test]
fn snapshot_orders_hud_then_overlay_then_input_holders_most_recent_on_top() {
    let mut manager = manager();
    let editor = [("Overlay", "Shared"), ("Input", "Shared")];
    let a = register(&mut manager, &descriptor("editor-a", &editor, ""), "editor-a");
    let b = register(&mut manager, &descriptor("editor-b", &editor, ""), "editor-b");
    let hud = register(&mut manager, &descriptor("hud", &[("Hud", "Shared")], ""), "hud");
    let panel = register(&mut manager, &descriptor("panel", &[("Overlay", "Shared")], ""), "panel");
    let steps = [
        (b, Capability::Input),
        (b, Capability::Overlay),
        (a, Capability::Overlay),
        (a, Capability::Input),
        (hud, Capability::Hud),
        (panel, Capability::Overlay),
    ];
    for (index, capability) in steps {
        manager.acquire(index, capability, 0).expect("known");
    }

    let order: Vec<(ModIndex, LayerKind, u8)> =
        layers(&manager).iter().map(|entry| (entry.mod_index, entry.kind, entry.allowed)).collect();
    assert_eq!(
        order,
        vec![
            (hud, LayerKind::Hud, HUD),
            (panel, LayerKind::Overlay, OVERLAY),
            (b, LayerKind::Overlay, OVERLAY),
            (a, LayerKind::Overlay, OVERLAY),
        ]
    );

    manager.release(b, Capability::Input);
    manager.acquire(b, Capability::Input, 0).expect("known");

    let top: Vec<ModIndex> = layers(&manager).iter().map(|entry| entry.mod_index).collect();
    assert_eq!(top, vec![hud, panel, a, b]);
}

#[test]
fn a_mod_with_hud_and_overlay_gets_two_layers_hud_below_every_overlay() {
    let mut manager = manager();
    let panel = register(&mut manager, &descriptor("panel", &[("Overlay", "Shared")], ""), "panel");
    let both = register(&mut manager, &descriptor("both", &[("Hud", "Shared"), ("Overlay", "Shared")], ""), "both");
    let corner = register(&mut manager, &descriptor("corner", &[("Hud", "Shared")], ""), "corner");
    for (index, capability) in
        [(both, Capability::Overlay), (both, Capability::Hud), (panel, Capability::Overlay), (corner, Capability::Hud)]
    {
        manager.acquire(index, capability, 0).expect("known");
    }

    let order: Vec<(ModIndex, LayerKind, u8)> =
        layers(&manager).iter().map(|entry| (entry.mod_index, entry.kind, entry.allowed)).collect();

    assert_eq!(
        order,
        vec![
            (both, LayerKind::Hud, HUD),
            (corner, LayerKind::Hud, HUD),
            (panel, LayerKind::Overlay, OVERLAY),
            (both, LayerKind::Overlay, OVERLAY),
        ]
    );
}

#[test]
fn a_suspended_hud_removes_only_the_hud_layer_of_that_mod() {
    let mut manager = manager();
    let both = [("Hud", "Shared"), ("Overlay", "Shared")];
    let shared = register(&mut manager, &descriptor("shared", &both, ""), "shared");
    let owner = register(&mut manager, &descriptor("owner", &[("Hud", "Exclusive")], ""), "owner");
    manager.acquire(shared, Capability::Hud, 0).expect("known");
    manager.acquire(shared, Capability::Overlay, 0).expect("known");

    manager.acquire(owner, Capability::Hud, 0).expect("known");
    manager.record_frame_reply(shared, LayerKind::Overlay, reply(true, false, [0.0, 0.0, 100.0, 100.0]));
    manager.route(&InputEvent::PointerMove { x: 50.0, y: 50.0 }, 0);
    let click = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 50.0, y: 50.0 }, 0);

    let order: Vec<(ModIndex, LayerKind, u8)> =
        layers(&manager).iter().map(|entry| (entry.mod_index, entry.kind, entry.allowed)).collect();
    assert_eq!(order, vec![(owner, LayerKind::Hud, HUD), (shared, LayerKind::Overlay, OVERLAY)]);
    assert!(click.raw_targets.contains(shared));
}

#[test]
fn a_hud_layer_that_wants_the_pointer_keeps_it_when_the_overlay_layer_of_the_mod_does_not() {
    let mut manager = manager();
    let both = [("Hud", "Shared"), ("Overlay", "Shared")];
    let shared = register(&mut manager, &descriptor("shared", &both, ""), "shared");
    manager.acquire(shared, Capability::Hud, 0).expect("known");
    manager.acquire(shared, Capability::Overlay, 0).expect("known");
    manager.record_frame_reply(shared, LayerKind::Hud, reply(true, true, [0.0, 0.0, 100.0, 100.0]));
    manager.record_frame_reply(shared, LayerKind::Overlay, reply(false, false, [0.0; 4]));

    manager.route(&InputEvent::PointerMove { x: 50.0, y: 50.0 }, 0);
    let click = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 50.0, y: 50.0 }, 0);
    let typed = manager.route(&InputEvent::Char(0x61), 0);

    assert!(click.raw_targets.contains(shared));
    assert!(typed.raw_targets.contains(shared));
}

#[test]
fn hud_claim_with_hide_vanilla_hud_hides_it_until_release() {
    let mut manager = manager();
    let minimap = register(&mut manager, MINIMAP, "minimap");
    let freecam = register(&mut manager, FREECAM, "freecam");

    manager.acquire(minimap, Capability::Hud, 0).expect("known");
    assert!(!manager.snapshot().hide_vanilla_hud);

    manager.acquire(freecam, Capability::Hud, 0).expect("known");
    assert!(manager.snapshot().hide_vanilla_hud);

    manager.release(freecam, Capability::Hud);
    assert!(!manager.snapshot().hide_vanilla_hud);
}

// A descriptor with one Input claim of the given mode, with or without block_player_turn.
fn input_holder(id: &str, mode: &str, blocks: bool) -> String {
    let flag = if blocks { "  block_player_turn = true\n" } else { "" };
    let extra = format!("claim \"Input\" {{\n  mode = \"{mode}\"\n  reason = \"r\"\n{flag}}}\n");
    descriptor(id, &[], &extra)
}

#[test]
fn the_player_turn_is_blocked_only_while_a_flagged_input_holder_is_active() {
    let mut manager = manager();
    let look = register(&mut manager, &input_holder("look", "Shared", true), "look");
    let plain = register(&mut manager, &input_holder("plain", "Shared", false), "plain");
    let owner = register(&mut manager, &input_holder("owner", "Exclusive", false), "owner");
    assert!(!manager.snapshot().blocks_player_turn);

    manager.acquire(plain, Capability::Input, 0).expect("known");
    assert!(!manager.snapshot().blocks_player_turn);

    manager.acquire(look, Capability::Input, 0).expect("known");
    assert!(manager.snapshot().blocks_player_turn);

    manager.acquire(owner, Capability::Input, 0).expect("known");
    assert!(!manager.snapshot().blocks_player_turn);

    manager.release(owner, Capability::Input);
    assert!(manager.snapshot().blocks_player_turn);

    manager.release(look, Capability::Input);
    assert!(!manager.snapshot().blocks_player_turn);
}

#[test]
fn block_player_turn_and_pass_pointer_motion_stay_independent() {
    let mut manager = manager();
    let extra = "claim \"Input\" {
  mode = \"Shared\"
  reason = \"r\"
  block_player_turn = true
  pass_pointer_motion = true
}
";
    let look = register(&mut manager, &descriptor("look", &[], extra), "look");
    manager.acquire(look, Capability::Input, 0).expect("known");

    let decision = manager.route(&InputEvent::PointerDelta { dx: 2.0, dy: 1.0 }, 0);

    assert!(manager.snapshot().blocks_player_turn);
    assert!(!decision.consume_from_game);
    assert_eq!(decision.raw_targets.iter().collect::<Vec<_>>(), vec![look]);
}

#[test]
fn set_setting_from_a_mod_does_not_send_settings_changed_to_that_mod() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    events(&mut manager, freecam);

    manager.set_setting(freecam, "keys.toggle", "\"Ctrl+O\"", 0).expect("valid");

    assert!(events(&mut manager, freecam).is_empty());
    let writes: Vec<WriteRequest> = manager.due_writes(0).collect();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].text.contains("toggle = \"Ctrl+O\""));
    let toggle = action(&manager, freecam, "toggle");
    assert_eq!(manager.route(&key(KEY_O, true, CTRL), 0).keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
}

#[test]
fn set_setting_refuses_an_unknown_path_a_bad_value_and_an_unknown_mod() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");

    let unknown = manager.set_setting(freecam, "keys.nope", "\"Ctrl+O\"", 0);
    let wrong_type = manager.set_setting(freecam, "keys.toggle", "5", 0);
    let not_hcl = manager.set_setting(freecam, "keys.toggle", "\"Ctrl+O", 0);

    assert!(matches!(unknown, Err(ManagerError::Setting { path }) if path == "keys.nope"));
    assert!(matches!(wrong_type, Err(ManagerError::Setting { path }) if path == "keys.toggle"));
    assert!(matches!(not_hcl, Err(ManagerError::Setting { path }) if path == "keys.toggle"));
    assert!(matches!(manager.set_setting(ModIndex(9), "keys.toggle", "\"O\"", 0), Err(ManagerError::UnknownMod)));
}

#[test]
fn burst_of_set_setting_gives_one_due_write_per_500_ms() {
    let mut manager = manager();
    let document = HclDocument::new("keys {\n  toggle = \"Ctrl+L\"\n}\n").expect("parses");
    let (levels, _) = manager.register(LEVELS, Some(document), &dir("levels")).expect("registers");

    manager.set_setting(levels, "keys.toggle", "\"Ctrl+A\"", 1000).expect("valid");
    assert_eq!(manager.due_writes(1000).count(), 1);

    manager.set_setting(levels, "keys.toggle", "\"Ctrl+B\"", 1100).expect("valid");
    manager.set_setting(levels, "keys.toggle", "\"Ctrl+C\"", 1200).expect("valid");
    assert_eq!(manager.due_writes(1200).count(), 0);
    assert_eq!(manager.due_writes(1499).count(), 0);

    let writes: Vec<WriteRequest> = manager.due_writes(1500).collect();
    assert_eq!(writes.len(), 1);
    assert!(writes[0].text.contains("\"Ctrl+C\""));
    assert_eq!(manager.due_writes(5000).count(), 0);
}

#[test]
fn write_table_evicts_only_idle_entries_and_keeps_pending_text() {
    let mut config = ManagerConfig::default();
    config.budgets.max_mods = 1;
    let mut manager = ManagerService::new(config, None);
    let valid = || {
        HclDocument::new(
            "keys {
  toggle = \"Ctrl+L\"
}
",
        )
        .expect("parses")
    };
    let (levels, _) = manager.register(LEVELS, Some(valid()), &dir("levels")).expect("registers");
    manager.set_setting(levels, "keys.toggle", "\"Ctrl+A\"", 0).expect("valid");
    assert_eq!(manager.due_writes(0).count(), 1);
    manager.unregister(levels);
    let minimap = register(&mut manager, MINIMAP, "minimap");
    assert_eq!(manager.due_writes(10).count(), 1);
    manager.unregister(minimap);
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    manager.unregister(dejavu);

    let (levels, _) = manager.register(LEVELS, Some(valid()), &dir("levels")).expect("registers");
    manager.set_setting(levels, "keys.toggle", "\"Ctrl+B\"", 100).expect("valid");

    // levels was written at 0, so only an evicted entry lets its next write go before 500
    let mut paths: Vec<PathBuf> = manager.due_writes(100).map(|write| write.path).collect();
    paths.sort();
    assert_eq!(paths, vec![dir("dejavu").join("config.hcl"), dir("levels").join("config.hcl")]);
}

#[test]
fn set_setting_right_after_an_auto_save_waits_for_the_merge_window() {
    let mut manager = manager();
    let document = HclDocument::new(
        "keys {
  toggle = \"Ctrl+L\"
}
",
    )
    .expect("parses");
    let (levels, _) = manager.register(LEVELS, Some(document), &dir("levels")).expect("registers");
    manager.edit_draft(levels, "keys.toggle", hcl::Value::from("Ctrl+A"), 500).expect("valid");
    assert_eq!(manager.due_writes(1000).count(), 1);

    manager.set_setting(levels, "keys.toggle", "\"Ctrl+B\"", 1100).expect("valid");

    assert_eq!(manager.due_writes(1100).count(), 0);
    assert_eq!(manager.due_writes(1500).count(), 1);
}

#[test]
fn an_edit_is_written_one_merge_window_after_it_with_no_save_action() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    manager.due_writes(0).count();
    events(&mut manager, freecam);

    manager.edit_draft(freecam, "keys.toggle", hcl::Value::from("Ctrl+O"), 1000).expect("valid");
    let early = manager.due_writes(1499).count();
    let writes: Vec<WriteRequest> = manager.due_writes(1500).collect();

    assert_eq!(early, 0);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].path, dir("freecam").join("config.hcl"));
    assert!(writes[0].text.contains("\"Ctrl+O\""));
    assert_eq!(events(&mut manager, freecam), vec![ModEvent::SettingsChanged { paths: 1 }]);
    assert!(!manager.draft()[usize::from(freecam.0)].is_dirty());
    assert!(matches!(
        manager.edit_draft(freecam, "keys.toggle", hcl::Value::from(5), 2000),
        Err(ManagerError::Setting { path }) if path == "keys.toggle"
    ));
}

#[test]
fn a_second_edit_within_the_window_merges_into_one_write() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let levels = register(&mut manager, LEVELS, "levels");
    manager.due_writes(0).count();
    events(&mut manager, freecam);
    events(&mut manager, levels);

    manager.edit_draft(freecam, "keys.toggle", hcl::Value::from("Ctrl+O"), 1000).expect("valid");
    manager.edit_draft(freecam, "keys.clear_path", hcl::Value::from("Ctrl+J"), 1300).expect("valid");
    let after_first = manager.due_writes(1500).count();
    let writes: Vec<WriteRequest> = manager.due_writes(1800).collect();

    assert_eq!(after_first, 0, "the second edit restarts the window");
    assert_eq!(writes.len(), 1);
    assert!(writes[0].text.contains("\"Ctrl+O\"") && writes[0].text.contains("\"Ctrl+J\""));
    assert_eq!(events(&mut manager, freecam), vec![ModEvent::SettingsChanged { paths: 2 }]);
    assert!(events(&mut manager, levels).is_empty());
    assert_eq!(manager.due_writes(10_000).count(), 0);
}

#[test]
fn save_status_goes_saving_then_saved_then_idle_and_failed_on_a_refused_write() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    manager.due_writes(0).count();
    let idle = manager.save_status(5000);

    manager.edit_draft(freecam, "keys.toggle", hcl::Value::from("Ctrl+O"), 5000).expect("valid");
    let saving = manager.save_status(5100);
    manager.due_writes(5500).count();
    let saved = (manager.save_status(5500), manager.save_status(7499));
    let faded = manager.save_status(7500);
    manager.edit_draft(freecam, "keys.toggle", hcl::Value::from("Ctrl+K"), 8000).expect("valid");
    let refused: Vec<WriteRequest> = manager.due_writes(8500).collect();
    manager.write_refused(refused[0].clone(), 8500);
    let failed = manager.save_status(8500);

    assert_eq!(idle, SaveStatus::Idle);
    assert_eq!(saving, SaveStatus::Saving);
    assert_eq!(saved, (SaveStatus::Saved, SaveStatus::Saved));
    assert_eq!(faded, SaveStatus::Idle);
    assert_eq!(failed, SaveStatus::Failed);
}

#[test]
fn thirtieth_frame_failure_marks_the_mod_failed_and_toasts() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    manager.acquire(freecam, Capability::Camera, 0).expect("known");
    manager.acquire(dejavu, Capability::Overlay, 0).expect("known");
    manager.acquire(dejavu, Capability::Camera, 0).expect("known");
    manager.set_cursor_mode(dejavu, true).expect("holds overlay");
    events(&mut manager, freecam);

    let early: Vec<bool> = (0..29).map(|ms| manager.record_frame_failure(dejavu, ms)).collect();
    assert!(early.iter().all(|failed| !failed));
    assert_eq!(layers(&manager).len(), 1);

    assert!(manager.record_frame_failure(dejavu, 30));

    assert!(layers(&manager).is_empty());
    assert!(manager.registry().get(dejavu).expect("registered").failed);
    assert_eq!(toasts(&manager), vec![ToastKind::FrameDisabled(dejavu)]);
    assert!(!manager.arbiter().holds(dejavu, Capability::Camera));
    assert_eq!(events(&mut manager, freecam), vec![ModEvent::Resumed(Capability::Camera)]);
    assert!(!manager.snapshot().cursor_free);
    assert!(manager.record_frame_failure(dejavu, 31));
    assert_eq!(toasts(&manager).len(), 1);

    manager.reset_failures(dejavu);
    manager.acquire(dejavu, Capability::Overlay, 40).expect("known");

    assert_eq!(layers(&manager).len(), 1);
    assert!(!manager.record_frame_failure(dejavu, 40));
}

#[test]
fn failures_must_be_consecutive_to_disable_a_mod() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");

    for ms in 0..29 {
        manager.record_frame_failure(freecam, ms);
    }
    manager.record_frame_success(freecam);
    let interrupted: Vec<bool> = (0..29).map(|ms| manager.record_frame_failure(freecam, ms)).collect();

    assert!(interrupted.iter().all(|failed| !failed));
    assert!(manager.record_frame_failure(freecam, 30));
}

#[test]
fn a_new_mod_in_a_reused_slot_gets_no_stray_key_up() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    manager.acquire(freecam, Capability::Input, 0).expect("known");
    manager.route(&key(KEY_G, true, NO_MODS), 0);
    manager.unregister(freecam);
    let minimap = register(&mut manager, MINIMAP, "minimap");
    assert_eq!(minimap, freecam);
    manager.acquire(minimap, Capability::Input, 0).expect("known");
    events(&mut manager, minimap);

    manager.route(&key(KEY_G, false, NO_MODS), 0);

    assert!(events(&mut manager, minimap).is_empty());
}

#[test]
fn key_down_is_false_for_a_mod_without_input() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let minimap = register(&mut manager, MINIMAP, "minimap");
    manager.acquire(freecam, Capability::Input, 0).expect("known");

    manager.route(&key(KEY_G, true, NO_MODS), 0);

    assert!(manager.key_down(freecam, KEY_G));
    assert!(!manager.key_down(minimap, KEY_G));
    assert!(!manager.key_down(freecam, KEY_H));
}

#[test]
fn world_loaded_sends_world_reload_apply_only_to_mods_with_pending_world_reload_changes() {
    let fields = "section \"view\" {\n  title = \"View\"\n  field \"view.radius\" {\n    label = \"Radius\"\n    \
                  kind = { int = { min = 1, max = 64, step = 1 } }\n    default = 8\n    apply = \"WorldReload\"\n  }\n  \
                  field \"view.grid\" {\n    label = \"Grid\"\n    kind = \"bool\"\n    default = false\n    \
                  apply = \"Live\"\n  }\n}\n";
    let mut manager = manager();
    let alpha = register(&mut manager, &descriptor("alpha", &[], fields), "alpha");
    let beta = register(&mut manager, &descriptor("beta", &[], fields), "beta");
    manager.edit_draft(alpha, "view.radius", hcl::Value::from(16), 0).expect("valid");
    manager.edit_draft(beta, "view.grid", hcl::Value::from(true), 0).expect("valid");
    manager.due_writes(500).count();
    assert_eq!(events(&mut manager, alpha), vec![ModEvent::SettingsChanged { paths: 1 }]);
    assert_eq!(events(&mut manager, beta), vec![ModEvent::SettingsChanged { paths: 1 }]);

    manager.world_loaded();

    assert_eq!(events(&mut manager, alpha), vec![ModEvent::WorldReloadApply]);
    assert!(events(&mut manager, beta).is_empty());

    manager.world_loaded();

    assert!(events(&mut manager, alpha).is_empty());
}

#[test]
fn acquire_denial_pushes_a_denied_toast() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    manager.acquire(dejavu, Capability::Camera, 0).expect("known");

    let outcome = manager.acquire(freecam, Capability::Camera, 0).expect("known");

    assert_eq!(outcome, AcquireOutcome::Denied { holder: dejavu });
    assert_eq!(
        toasts(&manager),
        vec![ToastKind::Denied { capability: Capability::Camera, holder: dejavu, requester: freecam }]
    );
    assert!(matches!(manager.acquire(ModIndex(9), Capability::Camera, 0), Err(ManagerError::UnknownMod)));
}

#[test]
fn blocked_table_follows_a_resolution_change() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let dejavu = register(&mut manager, DEJAVU, "dejavu");
    manager.acquire(dejavu, Capability::Camera, 0).expect("known");
    let toggle = action(&manager, freecam, "toggle");

    let blocked = manager.route(&key(KEY_P, true, CTRL), 0);
    manager.route(&key(KEY_P, false, CTRL), 0);

    let toast =
        ToastKind::Blocked { capability: Capability::Camera, holder: dejavu, mod_index: freecam, action: toggle };
    assert_eq!(blocked.blocked, Some(toast));
    assert!(blocked.consume_from_game);
    assert_eq!(toasts(&manager), vec![toast]);

    let pair = PairKey::new(Capability::Camera, "@alaydriem/freecam", "@alaydriem/dejavu");
    manager.set_suspension_resolution(pair, SuspensionResolution::Coexist);

    let pressed = manager.route(&key(KEY_P, true, CTRL), 0);
    assert_eq!(pressed.blocked, None);
    assert_eq!(pressed.keybind, Some((freecam, toggle, KeybindPhase::Pressed)));
}

#[test]
fn set_cursor_mode_needs_input_or_overlay() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");

    assert!(matches!(manager.set_cursor_mode(freecam, true), Err(ManagerError::NotPermitted)));
    manager.acquire(freecam, Capability::Overlay, 0).expect("known");
    manager.set_cursor_mode(freecam, true).expect("holds overlay");
    assert!(manager.snapshot().cursor_free);

    manager.release(freecam, Capability::Overlay);

    assert!(!manager.snapshot().cursor_free);
    manager.set_panel_open(true);
    assert!(manager.snapshot().cursor_free && manager.snapshot().panel_open);
}

#[test]
fn focus_lost_releases_every_held_key_to_its_receiver() {
    let mut manager = manager();
    let freecam = register(&mut manager, FREECAM, "freecam");
    let minimap = register(&mut manager, MINIMAP, "minimap");
    manager.acquire(freecam, Capability::Input, 0).expect("known");
    manager.acquire(minimap, Capability::Input, 0).expect("known");
    events(&mut manager, freecam);
    events(&mut manager, minimap);
    let fly_forward = action(&manager, freecam, "fly_forward");

    manager.route(&key(KEY_G, true, NO_MODS), 0);
    manager.route(&key(KEY_W, true, NO_MODS), 0);
    manager.route(&InputEvent::FocusLost, 0);

    let down = |vk| ModEvent::Key { vk, down: true, repeat: false };
    let up = |vk| ModEvent::Key { vk, down: false, repeat: false };
    let bind = |phase| ModEvent::Keybind { action: fly_forward, phase };
    assert_eq!(
        events(&mut manager, freecam),
        vec![down(KEY_G), bind(KeybindPhase::Pressed), up(KEY_G), bind(KeybindPhase::Released)]
    );
    assert_eq!(events(&mut manager, minimap), vec![down(KEY_G), up(KEY_G)]);
    assert!(!manager.key_down(freecam, KEY_G));

    manager.route(&key(KEY_G, false, NO_MODS), 0);

    assert!(events(&mut manager, freecam).is_empty());
}

#[test]
fn full_event_queue_drops_are_counted_in_telemetry() {
    let mut config = ManagerConfig::default();
    config.budgets.event_queue = 2;
    let mut manager = ManagerService::new(config, None);
    let freecam = register(&mut manager, FREECAM, "freecam");
    manager.acquire(freecam, Capability::Input, 0).expect("known");

    for vk in [KEY_G, KEY_H, KEY_J] {
        manager.route(&key(vk, true, NO_MODS), 0);
    }

    assert_eq!(manager.telemetry().event_drops(), 2);
    assert!(manager.telemetry().report().contains("event_drops=2"));
}

fn reply(wants_pointer: bool, wants_keyboard: bool, layer_rect: [f32; 4]) -> FrameReplyData {
    FrameReplyData { cursor: CursorShape::Default, wants_pointer, wants_keyboard, layer_rect }
}

#[test]
fn a_click_over_the_top_layer_that_wants_the_pointer_goes_to_that_mod() {
    let mut manager = manager();
    let below = register(&mut manager, &descriptor("below", &[("Overlay", "Shared")], ""), "below");
    let above = register(&mut manager, &descriptor("above", &[("Overlay", "Shared")], ""), "above");
    manager.acquire(below, Capability::Overlay, 0).expect("known");
    manager.acquire(above, Capability::Overlay, 0).expect("known");
    manager.record_frame_reply(below, LayerKind::Overlay, reply(true, false, [0.0, 0.0, 200.0, 200.0]));
    manager.record_frame_reply(above, LayerKind::Overlay, reply(true, false, [0.0, 0.0, 100.0, 100.0]));

    manager.route(&InputEvent::PointerMove { x: 50.0, y: 50.0 }, 0);
    let inside = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 50.0, y: 50.0 }, 0);
    manager.route(&InputEvent::PointerButton { button: 0, down: false, x: 50.0, y: 50.0 }, 0);
    let lower = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 150.0, y: 150.0 }, 0);
    manager.route(&InputEvent::PointerButton { button: 0, down: false, x: 150.0, y: 150.0 }, 0);
    let outside = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 500.0, y: 500.0 }, 0);

    assert!(inside.consume_from_game);
    assert!(inside.raw_targets.contains(above) && !inside.raw_targets.contains(below));
    assert!(lower.raw_targets.contains(below) && !lower.raw_targets.contains(above));
    assert!(outside.raw_targets.is_empty());
    assert!(!outside.consume_from_game);
}

#[test]
fn text_goes_to_the_mod_that_last_wanted_the_keyboard_until_it_stops_or_leaves() {
    let mut manager = manager();
    let typist = register(&mut manager, &descriptor("typist", &[("Overlay", "Shared")], ""), "typist");
    manager.acquire(typist, Capability::Overlay, 0).expect("known");
    manager.record_frame_reply(typist, LayerKind::Overlay, reply(false, true, [0.0; 4]));

    let typed = manager.route(&InputEvent::Char(0x61), 0);
    manager.record_frame_reply(typist, LayerKind::Overlay, reply(false, false, [0.0; 4]));
    let after_stop = manager.route(&InputEvent::Char(0x61), 0);
    manager.record_frame_reply(typist, LayerKind::Overlay, reply(false, true, [0.0; 4]));
    manager.release(typist, Capability::Overlay);
    let after_release = manager.route(&InputEvent::Char(0x61), 0);

    assert!(typed.raw_targets.contains(typist));
    assert!(after_stop.raw_targets.is_empty());
    assert!(after_release.raw_targets.is_empty());
}

#[test]
fn disabling_a_mod_writes_the_manager_config_and_keeps_comments() {
    let path = Path::new("manager").join("config.hcl");
    let (config, document) = ManagerConfigFile::read(
        "# operator note
telemetry = false
",
    )
    .unwrap_or_else(|error| panic!("{error}"));
    let mut manager = ManagerService::new(config, Some((path.clone(), document)));
    let freecam = register(&mut manager, FREECAM, "freecam");
    let pair = PairKey::new(Capability::Camera, "@alaydriem/freecam", "@alaydriem/dejavu");

    manager.set_enabled(freecam, false);
    manager.set_exclusive_resolution(pair.clone(), ExclusiveResolution::Prefer("@alaydriem/dejavu".into()));
    manager.set_suspension_resolution(pair.clone(), SuspensionResolution::Coexist);
    let writes: Vec<WriteRequest> = manager.due_writes(10_000).filter(|write| write.path == path).collect();

    assert_eq!(writes.len(), 1);
    assert!(writes[0].text.contains("# operator note"));
    let (reread, _) = ManagerConfigFile::read(&writes[0].text).unwrap_or_else(|error| panic!("{error}"));
    assert!(!reread.is_enabled("@alaydriem/freecam"));
    assert_eq!(reread.resolutions.exclusive_for(&pair), Some(&ExclusiveResolution::Prefer("@alaydriem/dejavu".into())));
    assert_eq!(reread.resolutions.suspension_for(&pair), Some(&SuspensionResolution::Coexist));
    assert!(!reread.telemetry);
}

#[test]
fn a_refused_write_toasts_and_is_queued_again() {
    let mut manager = manager();
    let write = WriteRequest {
        path: PathBuf::from("mods/a/config.hcl"),
        text: "a = 1
"
        .into(),
    };

    manager.write_refused(write.clone(), 1000);
    let early: Vec<WriteRequest> = manager.due_writes(1000).collect();
    let later: Vec<WriteRequest> = manager.due_writes(1000).collect();

    assert_eq!(toasts(&manager), vec![ToastKind::SaveFailed]);
    assert_eq!(early, vec![write]);
    assert!(later.is_empty());
}

#[test]
fn rebinding_the_panel_toggle_writes_toggle_chord_and_routes_the_new_chord() {
    let path = Path::new("manager").join("config.hcl");
    let mut manager = ManagerService::new(ManagerConfig::default(), Some((path.clone(), HclDocument::empty())));
    let ctrl_shift = Modifiers { ctrl: true, shift: true, alt: false };
    let chord = Chord { key: KEY_F9, ctrl: true, shift: true, alt: false, f3: false };

    manager.set_toggle_chord(chord);
    let writes: Vec<WriteRequest> = manager.due_writes(1000).filter(|write| write.path == path).collect();
    let pressed = manager.route(&key(KEY_F9, true, ctrl_shift), 1000);

    assert_eq!(manager.config().toggle_chord, chord);
    assert_eq!(writes.len(), 1);
    let (reread, _) = ManagerConfigFile::read(&writes[0].text).unwrap_or_else(|error| panic!("{error}"));
    assert_eq!(reread.toggle_chord, chord);
    assert!(pressed.toggle_panel);
}

#[test]
fn the_menu_button_shows_on_the_start_and_pause_screens_only() {
    let mut manager = manager();
    let mut shown_on = |screen: MenuScreen| {
        manager.set_menu_screen(screen);
        manager.menu_button_visible()
    };

    assert!(!shown_on(MenuScreen::None));
    assert!(shown_on(MenuScreen::Start));
    assert!(shown_on(MenuScreen::Pause));
}

#[test]
fn a_hud_layer_that_comes_back_does_not_keep_the_keyboard_it_wanted_before() {
    let mut manager = manager();
    let both = [("Hud", "Shared"), ("Overlay", "Shared")];
    let shared = register(&mut manager, &descriptor("shared", &both, ""), "shared");
    let owner = register(&mut manager, &descriptor("owner", &[("Hud", "Exclusive")], ""), "owner");
    manager.acquire(shared, Capability::Hud, 0).expect("known");
    manager.acquire(shared, Capability::Overlay, 0).expect("known");
    manager.record_frame_reply(shared, LayerKind::Hud, reply(true, true, [0.0, 0.0, 100.0, 100.0]));

    manager.acquire(owner, Capability::Hud, 0).expect("known");
    manager.release(owner, Capability::Hud);
    manager.route(&InputEvent::PointerMove { x: 50.0, y: 50.0 }, 0);
    let typed = manager.route(&InputEvent::Char(0x61), 0);
    let click = manager.route(&InputEvent::PointerButton { button: 0, down: true, x: 50.0, y: 50.0 }, 0);

    assert!(layers(&manager).iter().any(|entry| entry.mod_index == shared && entry.kind == LayerKind::Hud));
    assert!(!typed.raw_targets.contains(shared));
    assert!(!click.raw_targets.contains(shared));
}

const KEY_SHIFT: u32 = 0x10;
const KEY_F3: u32 = 0x72;

// A mod with a shared Input claim and the keybind "sneak" on F3+W.
fn toggles_mod() -> String {
    let extra = "claim \"Input\" {
  mode = \"Shared\"
  reason = \"r\"
}
keybind \"sneak\" {
  label       = \"Sneak\"
  default     = \"F3+W\"
  capability  = \"Input\"
  scope       = \"Global\"
  config_path = \"keys.sneak\"
}
section \"keys\" {
  title = \"Keys\"
  field \"keys.sneak\" {
    label   = \"Sneak\"
    kind    = \"chord\"
    default = \"F3+W\"
    apply   = \"Live\"
  }
}
";
    descriptor("toggles", &[], extra)
}

// The mod holds no Input lease, like a mod that only presses fake keys.
fn toggles(manager: &mut ManagerService) -> ModIndex {
    register(manager, &toggles_mod(), "toggles")
}

fn toggles_with_lease(manager: &mut ManagerService) -> ModIndex {
    let index = toggles(manager);
    manager.acquire(index, Capability::Input, 0).unwrap_or_else(|error| panic!("acquire: {error}"));
    events(manager, index);
    index
}

fn fake_ops(manager: &mut ManagerService) -> Vec<FakeKeyOp> {
    let mut out = Vec::new();
    manager.take_fake_keys(&mut out);
    out
}

fn fake_down(vk: u32) -> FakeKeyOp {
    FakeKeyOp { vk, down: true }
}

fn fake_up(vk: u32) -> FakeKeyOp {
    FakeKeyOp { vk, down: false }
}

#[test]
fn watched_key_reaches_watcher_and_game() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");

    let decision = manager.route(&key(KEY_W, true, NO_MODS), 0);

    assert!(!decision.consume_from_game);
    assert!(events(&mut manager, m).contains(&ModEvent::Key { vk: KEY_W, down: true, repeat: false }));
}

#[test]
fn watched_key_is_not_sent_twice_to_a_raw_target() {
    let mut manager = manager();
    let m = toggles_with_lease(&mut manager);
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");

    let decision = manager.route(&key(KEY_W, true, NO_MODS), 0);

    assert!(decision.raw_targets.contains(m));
    let keys = events(&mut manager, m).into_iter().filter(|event| matches!(event, ModEvent::Key { .. })).count();
    assert_eq!(keys, 1);
}

#[test]
fn keybind_press_is_not_sent_to_watchers() {
    let mut manager = manager();
    let m = toggles_with_lease(&mut manager);
    let sneak = action(&manager, m, "sneak");
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");
    manager.route(&key(KEY_F3, true, NO_MODS), 0);
    events(&mut manager, m);

    manager.route(&key(KEY_W, true, NO_MODS), 0);

    let got = events(&mut manager, m);
    assert!(got.contains(&ModEvent::Keybind { action: sneak, phase: KeybindPhase::Pressed }));
    assert!(!got.iter().any(|event| matches!(event, ModEvent::Key { vk: KEY_W, .. })));
}

#[test]
fn real_up_is_held_back_while_faked() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    let down = manager.route(&key(KEY_SHIFT, true, NO_MODS), 0);
    let up = manager.route(&key(KEY_SHIFT, false, NO_MODS), 0);

    assert!(!down.consume_from_game);
    assert!(up.consume_from_game);
}

#[test]
fn send_key_down_twice_queues_one_op() {
    let mut manager = manager();
    let m = toggles(&mut manager);

    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT)]);
}

#[test]
fn send_key_up_without_a_hold_queues_nothing() {
    let mut manager = manager();
    let m = toggles(&mut manager);

    manager.send_key(m, KEY_SHIFT, false).expect("claims Input");

    assert!(fake_ops(&mut manager).is_empty());
}

#[test]
fn send_key_up_while_real_key_down_sends_nothing() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_W, true).expect("claims Input");
    manager.route(&key(KEY_W, true, NO_MODS), 0);

    manager.send_key(m, KEY_W, false).expect("claims Input");
    let up = manager.route(&key(KEY_W, false, NO_MODS), 0);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_W)]);
    assert!(!up.consume_from_game);
}

#[test]
fn send_key_up_after_a_hold_queues_the_up() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_W, true).expect("claims Input");

    manager.send_key(m, KEY_W, false).expect("claims Input");

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_W), fake_up(KEY_W)]);
}

#[test]
fn chord_off_while_chord_key_held_queues_the_up() {
    let mut manager = manager();
    let m = toggles_with_lease(&mut manager);
    manager.send_key(m, KEY_W, true).expect("claims Input");
    manager.route(&key(KEY_F3, true, NO_MODS), 0);
    let down = manager.route(&key(KEY_W, true, NO_MODS), 0);
    assert!(down.consume_from_game);

    manager.send_key(m, KEY_W, false).expect("claims Input");

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_W), fake_up(KEY_W)]);
}

#[test]
fn release_all_while_swallowed_key_held_queues_the_up() {
    let mut manager = manager();
    let m = toggles_with_lease(&mut manager);
    manager.send_key(m, KEY_W, true).expect("claims Input");
    manager.route(&key(KEY_F3, true, NO_MODS), 0);
    manager.route(&key(KEY_W, true, NO_MODS), 0);

    manager.set_panel_open(true);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_W), fake_up(KEY_W)]);
}

#[test]
fn focus_lost_releases_and_notifies() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    manager.route(&InputEvent::FocusLost, 0);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
    assert!(events(&mut manager, m).contains(&ModEvent::FakeKeysReleased));
}

#[test]
fn release_keeps_the_up_back_while_the_real_key_is_down() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");
    manager.send_key(m, KEY_W, true).expect("claims Input");
    manager.route(&key(KEY_W, true, NO_MODS), 0);

    manager.world_left();

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_down(KEY_W), fake_up(KEY_SHIFT)]);
    assert!(events(&mut manager, m).contains(&ModEvent::FakeKeysReleased));
}

#[test]
fn panel_open_releases() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    manager.set_panel_open(true);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
    assert!(events(&mut manager, m).contains(&ModEvent::FakeKeysReleased));
}

#[test]
fn panel_close_keeps_fake_keys() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.set_panel_open(true);
    manager.set_panel_open(false);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    manager.set_panel_open(false);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT)]);
}

#[test]
fn world_left_releases() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    manager.world_left();

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
    assert!(events(&mut manager, m).contains(&ModEvent::FakeKeysReleased));
}

#[test]
fn disable_releases_and_forgets_watch() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");

    manager.set_enabled(m, false);
    manager.set_enabled(m, true);
    manager.route(&key(KEY_W, true, NO_MODS), 0);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
    let got = events(&mut manager, m);
    assert!(got.contains(&ModEvent::FakeKeysReleased));
    assert!(!got.iter().any(|event| matches!(event, ModEvent::Key { .. })));
}

#[test]
fn unregister_releases() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");

    manager.unregister(m);

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
}

#[test]
fn unregister_forgets_the_watch_for_a_new_mod_in_the_slot() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");
    manager.unregister(m);
    let next = toggles(&mut manager);

    manager.route(&key(KEY_W, true, NO_MODS), 0);

    assert!(!events(&mut manager, next).iter().any(|event| matches!(event, ModEvent::Key { .. })));
}

#[test]
fn send_key_without_input_claim_is_not_permitted() {
    let mut manager = manager();
    let m = register(&mut manager, &descriptor("plain", &[("Hud", "Shared")], ""), "plain");

    assert_eq!(manager.send_key(m, KEY_SHIFT, true), Err(ManagerError::NotPermitted));
    assert_eq!(manager.watch_keys(m, &[KEY_W]), Err(ManagerError::NotPermitted));
    assert!(fake_ops(&mut manager).is_empty());
}

#[test]
fn send_key_vk_256_is_invalid() {
    let mut manager = manager();
    let m = toggles(&mut manager);

    assert_eq!(manager.send_key(m, 256, true), Err(ManagerError::InvalidArgument));
    assert_eq!(manager.watch_keys(m, &[KEY_W, 256]), Err(ManagerError::InvalidArgument));
    assert_eq!(manager.watch_keys(m, &[KEY_W; 33]), Err(ManagerError::InvalidArgument));
    assert!(manager.watch_keys(m, &[KEY_W; 32]).is_ok());
}

#[test]
fn watch_keys_with_an_empty_slice_clears_the_set() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.watch_keys(m, &[KEY_W]).expect("claims Input");

    manager.watch_keys(m, &[]).expect("claims Input");
    manager.route(&key(KEY_W, true, NO_MODS), 0);

    assert!(!events(&mut manager, m).iter().any(|event| matches!(event, ModEvent::Key { .. })));
}

#[test]
fn send_key_while_panel_open_is_not_permitted() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.set_panel_open(true);

    assert_eq!(manager.send_key(m, KEY_SHIFT, true), Err(ManagerError::NotPermitted));
}

#[test]
fn send_key_while_another_mod_holds_input_exclusively_is_not_permitted() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    let owner = register(&mut manager, &input_holder("owner", "Exclusive", false), "owner");
    manager.acquire(owner, Capability::Input, 0).expect("known");

    assert_eq!(manager.send_key(m, KEY_SHIFT, true), Err(ManagerError::NotPermitted));
}

#[test]
fn send_key_while_other_mod_holds_input_is_not_permitted() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    let other = register(&mut manager, &input_holder("other", "Shared", false), "other");
    manager.acquire(other, Capability::Input, 0).unwrap_or_else(|error| panic!("acquire: {error}"));

    assert_eq!(manager.send_key(m, KEY_SHIFT, true), Err(ManagerError::NotPermitted));
    assert!(fake_ops(&mut manager).is_empty());
}

#[test]
fn input_lease_granted_to_other_releases_fake_keys() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");
    let other = register(&mut manager, &input_holder("other", "Shared", false), "other");

    manager.acquire(other, Capability::Input, 0).unwrap_or_else(|error| panic!("acquire: {error}"));

    assert_eq!(fake_ops(&mut manager), vec![fake_down(KEY_SHIFT), fake_up(KEY_SHIFT)]);
    assert!(events(&mut manager, m).contains(&ModEvent::FakeKeysReleased));
}

#[test]
fn send_key_up_allowed_while_other_holds_input() {
    let mut manager = manager();
    let m = toggles(&mut manager);
    manager.send_key(m, KEY_SHIFT, true).expect("claims Input");
    let other = register(&mut manager, &input_holder("other", "Shared", false), "other");
    manager.acquire(other, Capability::Input, 0).unwrap_or_else(|error| panic!("acquire: {error}"));

    assert_eq!(manager.send_key(m, KEY_SHIFT, false), Ok(()));
}
