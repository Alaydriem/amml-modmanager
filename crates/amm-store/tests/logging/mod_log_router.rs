use amm_core::errors::StoreError;
use amm_store::logging::{LogRoot, ModLogRouter, SessionStamp};
use curia::{Fields, Level, LogEvent};
use time::macros::datetime;

fn event(level: Level, message: &str) -> LogEvent {
    LogEvent {
        level,
        target: "test".into(),
        message: message.into(),
        fields: Fields::new(),
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    }
}

fn stamp() -> SessionStamp {
    SessionStamp::new(datetime!(2026-10-20 13:05:22.123 UTC))
}

fn router(game: &std::path::Path) -> ModLogRouter {
    ModLogRouter::new(LogRoot::new(game), stamp(), usize::from(ModLogRouter::MANAGER_SLOT) + 1)
}

fn log_text(game: &std::path::Path, name: &str) -> String {
    let path = game.join("logs").join("amm").join("@alaydriem").join(name).join(stamp().file_name());
    std::fs::read_to_string(path).unwrap_or_else(|error| format!("unreadable: {error}"))
}

#[test]
fn router_writes_each_mods_events_to_its_own_session_file() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    router.open(0, "alaydriem", "minimap", Level::Info).unwrap();
    router.open(1, "alaydriem", "freecam", Level::Info).unwrap();

    router.emit_for(0, &event(Level::Info, "from minimap"));
    router.emit_for(1, &event(Level::Info, "from freecam"));
    router.close(0);
    router.close(1);

    let minimap = log_text(game.path(), "minimap");
    let freecam = log_text(game.path(), "freecam");
    assert!(minimap.contains("from minimap") && !minimap.contains("from freecam"));
    assert!(freecam.contains("from freecam") && !freecam.contains("from minimap"));
}

#[test]
fn router_refuses_a_sink_past_the_manager_slot() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    let past = ModLogRouter::MANAGER_SLOT + 1;
    for slot in 0..past {
        router.open(slot, "alaydriem", &format!("mod{slot}"), Level::Info).unwrap();
    }

    let result = router.open(past, "alaydriem", "mod33", Level::Info);

    assert_eq!(result, Err(StoreError::Unavailable));
    assert!(!game.path().join("logs").join("amm").join("@alaydriem").join("mod33").exists());
    router.emit_for(past, &event(Level::Info, "ignored"));
    router.close(past);
    assert_eq!(router.dropped(17), 0);
}

#[test]
fn closing_a_slot_drains_its_queue_to_disk() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    router.open(3, "alaydriem", "levels", Level::Info).unwrap();

    for index in 0..300 {
        router.emit_for(3, &event(Level::Info, &format!("line {index}")));
    }
    router.close(3);

    let text = log_text(game.path(), "levels");
    assert_eq!(text.lines().count(), 300);
    assert!(text.lines().last().unwrap().contains("line 299"));
}

#[test]
fn events_more_verbose_than_the_slot_level_are_not_written() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    router.open(0, "alaydriem", "minimap", Level::Warn).unwrap();

    router.emit_for(0, &event(Level::Debug, "noise"));
    router.emit_for(0, &event(Level::Error, "problem"));
    router.close(0);

    let text = log_text(game.path(), "minimap");
    assert!(!text.contains("noise"));
    assert!(text.contains("problem"));
}

#[test]
fn reopening_an_occupied_slot_keeps_the_earlier_lines() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    router.open(2, "alaydriem", "dejavu", Level::Info).unwrap();
    router.emit_for(2, &event(Level::Info, "first"));
    router.close(2);
    router.open(2, "alaydriem", "dejavu", Level::Info).unwrap();
    assert_eq!(router.replaced(), 0);

    router.open(2, "alaydriem", "dejavu", Level::Info).unwrap();
    router.emit_for(2, &event(Level::Info, "second"));
    router.close(2);

    assert_eq!(router.replaced(), 1);

    let text = log_text(game.path(), "dejavu");
    assert_eq!(text.lines().count(), 2);
    assert!(text.contains("first") && text.contains("second"));
}

#[test]
fn a_mod_logs_under_its_namespace_folder() {
    let game = tempfile::tempdir().unwrap();
    let root = LogRoot::new(game.path());

    let dir = root.dir_for("alaydriem", "zoom");

    assert_eq!(dir, game.path().join("logs").join("amm").join("@alaydriem").join("zoom"));
}

#[test]
fn a_namespace_or_name_cannot_leave_the_log_root() {
    let game = tempfile::tempdir().unwrap();
    let root = LogRoot::new(game.path());
    let amm = game.path().join("logs").join("amm");

    let dir = root.dir_for("../..", "../escape");

    assert!(dir.starts_with(&amm));
    assert_eq!(dir.parent().unwrap().parent().unwrap(), amm);
    assert_eq!(root.dir_for("", "").strip_prefix(&amm).unwrap().components().count(), 2);
}

#[test]
fn a_session_over_the_old_limit_keeps_every_line() {
    let game = tempfile::tempdir().unwrap();
    let router = router(game.path());
    router.open(4, "alaydriem", "minimap", Level::Info).unwrap();

    for index in 0..1500 {
        router.emit_for(4, &event(Level::Info, &format!("line {index} padding padding padding padding padding")));
    }
    router.close(4);

    let dir = game.path().join("logs").join("amm").join("@alaydriem").join("minimap");
    let total_bytes: u64 =
        std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.metadata().map_or(0, |m| m.len())).sum();
    let lines: usize = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default().lines().count())
        .sum();
    assert!(total_bytes > 40_000);
    assert_eq!(lines, 1500);
}
