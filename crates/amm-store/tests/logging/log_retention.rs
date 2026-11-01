use std::path::Path;

use amm_store::logging::{LogRetention, SessionStamp};
use time::macros::datetime;

fn stamp_name(second: u8) -> String {
    let date = datetime!(2026-03-04 05:06:00 UTC) + time::Duration::seconds(i64::from(second));
    SessionStamp::new(date).file_name()
}

fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

#[test]
fn retention_keeps_the_newest_twenty_session_files() {
    let dir = tempfile::tempdir().unwrap();
    for second in 0..25 {
        std::fs::write(dir.path().join(stamp_name(second)), "x").unwrap();
    }

    let removed = LogRetention::prune(dir.path(), 20).unwrap();

    assert_eq!(removed, 5);
    let kept = names(dir.path());
    assert_eq!(kept.len(), 20);
    assert_eq!(kept.first().unwrap(), &stamp_name(5));
    assert_eq!(kept.last().unwrap(), &stamp_name(24));
}

#[test]
fn files_that_are_not_session_logs_are_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    for second in 0..4 {
        std::fs::write(dir.path().join(stamp_name(second)), "x").unwrap();
    }
    for other in ["notes.txt", "2026-03-04_05-06-00-000.log.1", "latest.log", "2026-03-04_05-06-00-000.txt"] {
        std::fs::write(dir.path().join(other), "x").unwrap();
    }

    let removed = LogRetention::prune(dir.path(), 2).unwrap();

    assert_eq!(removed, 2);
    assert_eq!(names(dir.path()).len(), 4 + 2);
    assert!(dir.path().join("notes.txt").exists());
    assert!(dir.path().join("latest.log").exists());
}

#[test]
fn the_current_session_file_is_never_deleted() {
    let dir = tempfile::tempdir().unwrap();
    for second in 0..5 {
        std::fs::write(dir.path().join(stamp_name(second)), "x").unwrap();
    }
    let current = SessionStamp::parse(stamp_name(0).trim_end_matches(".log")).unwrap();

    let removed = LogRetention::prune_keeping(dir.path(), 2, &current).unwrap();

    assert_eq!(removed, 3 - 1);
    assert!(dir.path().join(stamp_name(0)).exists());
    assert!(dir.path().join(stamp_name(4)).exists());
    assert!(dir.path().join(stamp_name(3)).exists());
    assert!(!dir.path().join(stamp_name(1)).exists());
}

#[test]
fn a_missing_directory_prunes_nothing() {
    let dir = tempfile::tempdir().unwrap();

    assert_eq!(LogRetention::prune(&dir.path().join("absent"), 20).unwrap(), 0);
}

#[test]
fn retention_removes_a_pruned_sessions_archives() {
    let dir = tempfile::tempdir().unwrap();
    let old = stamp_name(0);
    let old_stem = old.trim_end_matches(".log");
    let archives =
        [format!("{old_stem}_2026-03-04_05-06-30-000.log"), format!("{old_stem}_2026-03-04_05-06-30-000-2.log")];
    for name in [old.clone(), archives[0].clone(), archives[1].clone(), stamp_name(1), stamp_name(2)] {
        std::fs::write(dir.path().join(name), "x").unwrap();
    }
    let current_archive = format!("{}_2026-03-04_05-07-00-000.log", stamp_name(2).trim_end_matches(".log"));
    std::fs::write(dir.path().join(&current_archive), "x").unwrap();

    let removed = LogRetention::prune(dir.path(), 2).unwrap();

    assert_eq!(removed, 3);
    assert_eq!(names(dir.path()), {
        let mut kept = vec![stamp_name(1), stamp_name(2), current_archive];
        kept.sort();
        kept
    });
}

#[test]
fn the_current_sessions_archives_are_never_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let old = stamp_name(0);
    let archive = format!("{}_2026-03-04_05-06-30-000.log", old.trim_end_matches(".log"));
    for name in [old.clone(), archive.clone(), stamp_name(1), stamp_name(2)] {
        std::fs::write(dir.path().join(name), "x").unwrap();
    }
    let current = SessionStamp::parse(old.trim_end_matches(".log")).unwrap();

    LogRetention::prune_keeping(dir.path(), 1, &current).unwrap();

    assert!(dir.path().join(old).exists());
    assert!(dir.path().join(archive).exists());
    assert!(!dir.path().join(stamp_name(1)).exists());
}
