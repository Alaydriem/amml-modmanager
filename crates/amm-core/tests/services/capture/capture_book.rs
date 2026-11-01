use std::path::{Path, PathBuf};

use amm_core::config::ManagerConfig;
use amm_core::errors::CaptureError;
use amm_core::services::capture::CaptureBook;
use amm_core::services::manager::ManagerService;
use amm_core::structs::capture::ScreenshotLayers;
use amm_core::structs::descriptor::Capability;
use amm_core::structs::event::ModEvent;
use amm_core::structs::input::{InputEvent, Modifiers};
use amm_core::structs::registry::ModIndex;

const NO_MODS: Modifiers = Modifiers { ctrl: false, shift: false, alt: false };

fn manager() -> ManagerService {
    ManagerService::new(ManagerConfig::default(), None)
}

// A descriptor with the given claims (capability, mode).
fn descriptor(id: &str, claims: &[(&str, &str)]) -> String {
    let mut text = format!(
        "namespace = \"test\"\nname = \"{id}\"\ndisplay_name = \"{id}\"\nversion = \"0.1.0\"\nauthor = \"A\"\nconfig_path = \"config.hcl\"\n"
    );
    for (capability, mode) in claims {
        text.push_str(&format!("claim \"{capability}\" {{\n  mode = \"{mode}\"\n  reason = \"r\"\n}}\n"));
    }
    text
}

fn register(manager: &mut ManagerService, id: &str, claims: &[(&str, &str)]) -> ModIndex {
    let dir = Path::new("mods").join(id);
    let registered = manager.register(&descriptor(id, claims), None, &dir);
    registered.unwrap_or_else(|error| panic!("{id} does not register: {error}")).0
}

fn overlay_mod(manager: &mut ManagerService, id: &str) -> ModIndex {
    let index = register(manager, id, &[("Overlay", "Shared")]);
    manager.acquire(index, Capability::Overlay, 0).unwrap_or_else(|error| panic!("{id} does not acquire: {error}"));
    index
}

fn shot(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn events(manager: &mut ManagerService, index: ModIndex) -> Vec<ModEvent> {
    let mut out = [ModEvent::Enabled; 64];
    let count = manager.poll_events(index, &mut out);
    out[..count].to_vec()
}

fn screenshot_events(manager: &mut ManagerService, index: ModIndex) -> Vec<ModEvent> {
    let all = events(manager, index);
    all.into_iter()
        .filter(|event| matches!(event, ModEvent::ScreenshotTaken { .. } | ModEvent::ScreenshotWritten { .. }))
        .collect()
}

fn pending_sequences(manager: &ManagerService) -> Vec<u32> {
    manager.pending_captures().as_slice().iter().map(|request| request.sequence).collect()
}

// Takes every pending capture and reports each one copied and written.
fn finish_all(manager: &mut ManagerService) {
    let taken = manager.take_captures();
    for request in taken.as_slice() {
        manager.capture_taken(request.sequence, true);
        manager.capture_written(request.sequence, true);
    }
}

#[test]
fn a_capture_needs_an_active_hud_or_overlay_lease() {
    let mut manager = manager();
    let claims = [("Hud", "Shared"), ("Overlay", "Shared"), ("Camera", "Exclusive")];
    let first = register(&mut manager, "first", &claims);
    let owner = register(&mut manager, "owner", &[("Hud", "Exclusive")]);
    let overlay = register(&mut manager, "overlay", &[("Overlay", "Shared")]);

    let none = manager.request_capture(first, &shot("a.png"), ScreenshotLayers::GameOnly);
    manager.acquire(first, Capability::Camera, 0).expect("known");
    let camera_only = manager.request_capture(first, &shot("a.png"), ScreenshotLayers::GameOnly);
    manager.acquire(first, Capability::Hud, 0).expect("known");
    let with_hud = manager.request_capture(first, &shot("a.png"), ScreenshotLayers::GameOnly);
    finish_all(&mut manager);
    manager.acquire(overlay, Capability::Overlay, 0).expect("known");
    let with_overlay = manager.request_capture(overlay, &shot("b.png"), ScreenshotLayers::GameOnly);
    finish_all(&mut manager);
    manager.acquire(owner, Capability::Hud, 0).expect("known");
    let suspended = manager.request_capture(first, &shot("a.png"), ScreenshotLayers::GameOnly);
    let unknown = manager.request_capture(ModIndex(30), &shot("a.png"), ScreenshotLayers::GameOnly);

    assert_eq!(none, Err(CaptureError::NotPermitted));
    assert_eq!(camera_only, Err(CaptureError::NotPermitted));
    assert!(with_hud.is_ok());
    assert!(with_overlay.is_ok());
    assert!(manager.arbiter().is_suspended(first, Capability::Hud));
    assert_eq!(suspended, Err(CaptureError::NotPermitted));
    assert!(manager.request_capture(owner, &shot("c.png"), ScreenshotLayers::GameOnly).is_ok());
    assert_eq!(unknown, Err(CaptureError::UnknownMod));
}

#[test]
fn a_second_capture_of_one_mod_is_busy_until_written() {
    let mut manager = manager();
    let index = overlay_mod(&mut manager, "shooter");

    let first = manager.request_capture(index, &shot("one.png"), ScreenshotLayers::GameOnly).expect("first capture");
    let while_pending = manager.request_capture(index, &shot("two.png"), ScreenshotLayers::GameOnly);
    let taken = manager.take_captures();
    let while_taken = manager.request_capture(index, &shot("two.png"), ScreenshotLayers::GameOnly);
    let waits_for_write = manager.capture_taken(first, true);
    let while_writing = manager.request_capture(index, &shot("two.png"), ScreenshotLayers::GameOnly);
    let path = manager.capture_path(first).map(Path::to_path_buf);
    manager.capture_written(first, true);
    let after =
        manager.request_capture(index, &shot("two.png"), ScreenshotLayers::GameOnly).expect("capture after the write");

    assert_eq!(while_pending, Err(CaptureError::Busy));
    assert_eq!(taken.as_slice().len(), 1);
    assert_eq!(taken.as_slice()[0].sequence, first);
    assert_eq!(taken.as_slice()[0].mod_index, index);
    assert_eq!(while_taken, Err(CaptureError::Busy));
    assert!(waits_for_write);
    assert_eq!(while_writing, Err(CaptureError::Busy));
    assert_eq!(path, Some(shot("one.png")));
    assert_ne!(after, first);
    assert_eq!(pending_sequences(&manager), vec![after]);
}

#[test]
fn a_taken_capture_is_not_taken_again_and_a_failed_copy_ends_it() {
    let mut manager = manager();
    let index = overlay_mod(&mut manager, "shooter");
    let sequence = manager.request_capture(index, &shot("one.png"), ScreenshotLayers::GameOnly).expect("capture");

    let first = manager.take_captures();
    let second = manager.take_captures();
    let waits_for_write = manager.capture_taken(sequence, false);
    let again = manager.request_capture(index, &shot("two.png"), ScreenshotLayers::GameOnly);

    assert_eq!(first.as_slice().len(), 1);
    assert!(second.as_slice().is_empty());
    assert!(!waits_for_write);
    assert!(manager.capture_path(sequence).is_none());
    assert_eq!(screenshot_events(&mut manager, index), vec![ModEvent::ScreenshotTaken { sequence, ok: false }]);
    assert!(again.is_ok());
}

#[test]
fn the_fifth_capture_in_flight_is_busy() {
    let mut manager = manager();
    let mods: Vec<ModIndex> = (0..5).map(|n| overlay_mod(&mut manager, &format!("mod-{n}"))).collect();

    let granted: Vec<u32> = mods[..4]
        .iter()
        .map(|index| {
            manager.request_capture(*index, &shot("many.png"), ScreenshotLayers::GameOnly).expect("one of four")
        })
        .collect();
    let fifth = manager.request_capture(mods[4], &shot("many.png"), ScreenshotLayers::GameOnly);
    manager.take_captures();
    manager.capture_taken(granted[0], true);
    let fifth_while_writing = manager.request_capture(mods[4], &shot("many.png"), ScreenshotLayers::GameOnly);
    manager.capture_written(granted[0], true);
    let fifth_after = manager.request_capture(mods[4], &shot("many.png"), ScreenshotLayers::GameOnly);

    assert_eq!(fifth, Err(CaptureError::Busy));
    assert_eq!(fifth_while_writing, Err(CaptureError::Busy));
    assert!(fifth_after.is_ok());
}

#[test]
fn a_path_that_is_relative_not_png_or_has_dot_dot_is_refused() {
    let mut manager = manager();
    let index = overlay_mod(&mut manager, "shooter");
    let temp = std::env::temp_dir();
    let refused = [
        PathBuf::from("shot.png"),
        temp.join("shot.jpg"),
        temp.join("shot"),
        temp.join("..").join("shot.png"),
        PathBuf::from(r"\\server\share\shot.png"),
        PathBuf::from(r"\\?\C:\shot.png"),
        PathBuf::from(r"\\.\C:\shot.png"),
        PathBuf::new(),
    ];

    let outcomes: Vec<_> =
        refused.iter().map(|path| manager.request_capture(index, path, ScreenshotLayers::GameOnly)).collect();
    let upper_case = manager.request_capture(index, &temp.join("SHOT.PNG"), ScreenshotLayers::GameOnly);

    for (path, outcome) in refused.iter().zip(&outcomes) {
        assert_eq!(*outcome, Err(CaptureError::InvalidPath), "{} was not refused", path.display());
    }
    assert!(upper_case.is_ok());
    assert_eq!(manager.pending_captures().as_slice().len(), 1);
}

#[test]
fn taken_and_written_events_reach_the_mod_in_order_and_survive_a_full_queue() {
    let mut config = ManagerConfig::default();
    config.budgets.event_queue = 16;
    let mut manager = ManagerService::new(config, None);
    let index = register(&mut manager, "shooter", &[("Input", "Shared"), ("Overlay", "Shared")]);
    manager.acquire(index, Capability::Input, 0).expect("known");
    manager.acquire(index, Capability::Overlay, 0).expect("known");
    events(&mut manager, index);
    let sequence = manager.request_capture(index, &shot("full.png"), ScreenshotLayers::GameOnly).expect("capture");
    manager.take_captures();

    for vk in 0x41..0x41 + 20 {
        manager.route(&InputEvent::Key { vk, down: true, repeat: false, mods: NO_MODS }, 0);
    }
    manager.capture_taken(sequence, true);
    for vk in 0x61..0x61 + 20 {
        manager.route(&InputEvent::Key { vk, down: true, repeat: false, mods: NO_MODS }, 0);
    }
    manager.capture_written(sequence, true);
    let got = screenshot_events(&mut manager, index);

    assert!(manager.telemetry().event_drops() > 0);
    assert_eq!(
        got,
        vec![ModEvent::ScreenshotTaken { sequence, ok: true }, ModEvent::ScreenshotWritten { sequence, ok: true }]
    );
}

#[test]
fn unregister_or_lease_loss_cancels_a_pending_capture() {
    let mut manager = manager();
    let released = overlay_mod(&mut manager, "released");
    let suspended = register(&mut manager, "suspended", &[("Hud", "Shared")]);
    let owner = register(&mut manager, "owner", &[("Hud", "Exclusive")]);
    let gone = overlay_mod(&mut manager, "gone");
    let copied = overlay_mod(&mut manager, "copied");
    manager.acquire(suspended, Capability::Hud, 0).expect("known");

    let copied_seq = manager.request_capture(copied, &shot("copied.png"), ScreenshotLayers::GameOnly).expect("capture");
    manager.take_captures();
    let released_seq =
        manager.request_capture(released, &shot("released.png"), ScreenshotLayers::GameOnly).expect("capture");
    let suspended_seq =
        manager.request_capture(suspended, &shot("suspended.png"), ScreenshotLayers::GameOnly).expect("capture");
    manager.request_capture(gone, &shot("gone.png"), ScreenshotLayers::GameOnly).expect("capture");
    manager.release(released, Capability::Overlay);
    manager.acquire(owner, Capability::Hud, 0).expect("known");
    manager.unregister(gone);
    manager.release(copied, Capability::Overlay);
    let copied_waits = manager.capture_taken(copied_seq, true);
    let late = [overlay_mod(&mut manager, "late-a"), overlay_mod(&mut manager, "late-b")];
    let others: Vec<u32> = late
        .iter()
        .map(|index| {
            manager
                .request_capture(*index, &shot("late.png"), ScreenshotLayers::GameOnly)
                .expect("room after the cancels")
        })
        .collect();

    assert_eq!(
        screenshot_events(&mut manager, released),
        vec![ModEvent::ScreenshotTaken { sequence: released_seq, ok: false }]
    );
    assert_eq!(
        screenshot_events(&mut manager, suspended),
        vec![ModEvent::ScreenshotTaken { sequence: suspended_seq, ok: false }]
    );
    assert!(manager.capture_path(released_seq).is_none());
    assert!(copied_waits);
    assert_eq!(
        screenshot_events(&mut manager, copied),
        vec![ModEvent::ScreenshotTaken { sequence: copied_seq, ok: true }]
    );
    assert_eq!(pending_sequences(&manager), others);
}

#[test]
fn a_capture_of_an_unregistered_mod_sends_no_event_to_the_next_mod_in_its_slot() {
    let mut manager = manager();
    let gone = overlay_mod(&mut manager, "gone");
    let sequence = manager.request_capture(gone, &shot("gone.png"), ScreenshotLayers::GameOnly).expect("capture");
    manager.take_captures();

    manager.unregister(gone);
    let next = overlay_mod(&mut manager, "next");
    events(&mut manager, next);
    let waits_for_write = manager.capture_taken(sequence, true);
    manager.capture_written(sequence, true);
    let next_capture = manager.request_capture(next, &shot("next.png"), ScreenshotLayers::GameOnly);

    assert_eq!(next, gone);
    assert!(!waits_for_write);
    assert!(screenshot_events(&mut manager, next).is_empty());
    assert!(next_capture.is_ok());
}

#[test]
fn a_pending_capture_the_host_never_takes_fails_after_its_tick_limit() {
    let mut manager = manager();
    let shooter = overlay_mod(&mut manager, "shooter");
    let sequence = manager.request_capture(shooter, &shot("a.png"), ScreenshotLayers::GameOnly).expect("booked");
    events(&mut manager, shooter);

    for _ in 1..CaptureBook::PENDING_TICKS {
        manager.tick_captures();
    }
    let before_limit = (screenshot_events(&mut manager, shooter), pending_sequences(&manager));
    manager.tick_captures();

    assert_eq!(before_limit, (vec![], vec![sequence]));
    assert_eq!(screenshot_events(&mut manager, shooter), vec![ModEvent::ScreenshotTaken { sequence, ok: false }]);
    assert!(pending_sequences(&manager).is_empty());
    assert!(manager.request_capture(shooter, &shot("b.png"), ScreenshotLayers::GameOnly).is_ok());
}

#[test]
fn ticks_do_not_end_a_capture_the_host_has_taken() {
    let mut manager = manager();
    let shooter = overlay_mod(&mut manager, "shooter");
    let sequence = manager.request_capture(shooter, &shot("a.png"), ScreenshotLayers::GameOnly).expect("booked");
    manager.take_captures();

    for _ in 0..CaptureBook::PENDING_TICKS * 2 {
        manager.tick_captures();
    }

    assert!(screenshot_events(&mut manager, shooter).is_empty());
    assert!(manager.capture_taken(sequence, true));
}

#[test]
fn a_capture_taken_before_its_mod_loses_the_lease_is_still_written() {
    let mut manager = manager();
    let shooter = overlay_mod(&mut manager, "shooter");
    let sequence = manager.request_capture(shooter, &shot("a.png"), ScreenshotLayers::GameOnly).expect("booked");
    manager.take_captures();

    manager.release(shooter, Capability::Overlay);
    let writes = manager.capture_taken(sequence, true);
    let path = manager.capture_path(sequence).map(Path::to_path_buf);
    manager.capture_written(sequence, true);

    assert!(writes);
    assert_eq!(path, Some(shot("a.png")));
    assert_eq!(
        screenshot_events(&mut manager, shooter),
        vec![ModEvent::ScreenshotTaken { sequence, ok: true }, ModEvent::ScreenshotWritten { sequence, ok: true }]
    );
}

#[test]
fn captures_with_and_without_layers_share_the_slot_count_and_the_one_per_mod_rule() {
    let mut manager = manager();
    let mods: Vec<ModIndex> = (0..5).map(|n| overlay_mod(&mut manager, &format!("mod-{n}"))).collect();

    let game = manager.request_capture(mods[0], &shot("game.png"), ScreenshotLayers::GameOnly).expect("game only");
    let second_of_one_mod = manager.request_capture(mods[0], &shot("layers.png"), ScreenshotLayers::WithModLayers);
    let layered: Vec<u32> = mods[1..4]
        .iter()
        .map(|index| manager.request_capture(*index, &shot("layers.png"), ScreenshotLayers::WithModLayers))
        .map(|outcome| outcome.expect("one of the four slots"))
        .collect();
    let fifth = manager.request_capture(mods[4], &shot("layers.png"), ScreenshotLayers::WithModLayers);
    let pending = manager.pending_captures();
    let game_set: Vec<u32> = pending.of(ScreenshotLayers::GameOnly).map(|request| request.sequence).collect();
    let layer_set: Vec<u32> = pending.of(ScreenshotLayers::WithModLayers).map(|request| request.sequence).collect();

    assert_eq!(second_of_one_mod, Err(CaptureError::Busy));
    assert_eq!(fifth, Err(CaptureError::Busy));
    assert_eq!(game_set, vec![game]);
    assert_eq!(layer_set, layered);
    assert_eq!(pending.as_slice().len(), 4);
}
