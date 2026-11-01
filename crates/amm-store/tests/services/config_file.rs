use amm_store::services::ConfigFile;
use amm_store::structs::ReadOutcome;

#[test]
fn missing_file_reads_as_missing() {
    let dir = tempfile::tempdir().unwrap();

    let outcome = ConfigFile::read(&dir.path().join("config.hcl"));

    assert!(matches!(outcome, ReadOutcome::Missing));
}

#[test]
fn valid_file_reads_as_parsed_and_leaves_no_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::write(&path, "zoom = 2\n").unwrap();

    let outcome = ConfigFile::read(&path);

    assert!(matches!(outcome, ReadOutcome::Parsed(_)));
    assert!(!dir.path().join("config.hcl.invalid").exists());
}

#[test]
fn invalid_hcl_is_copied_to_dot_invalid_and_reads_as_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::write(&path, "zoom = = [\n").unwrap();

    let outcome = ConfigFile::read(&path);

    assert!(matches!(outcome, ReadOutcome::Invalid));
    assert_eq!(std::fs::read_to_string(dir.path().join("config.hcl.invalid")).unwrap(), "zoom = = [\n");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "zoom = = [\n");
}

#[test]
fn a_newer_invalid_file_replaces_the_older_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::write(dir.path().join("config.hcl.invalid"), "old = = \n").unwrap();
    std::fs::write(&path, "new = = \n").unwrap();

    ConfigFile::read(&path);

    assert_eq!(std::fs::read_to_string(dir.path().join("config.hcl.invalid")).unwrap(), "new = = \n");
}

#[test]
fn file_that_is_not_utf8_reads_as_invalid() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::write(&path, [0xFF, 0xFE, 0x00, 0x80]).unwrap();

    let outcome = ConfigFile::read(&path);

    assert!(matches!(outcome, ReadOutcome::Invalid));
    assert!(dir.path().join("config.hcl.invalid").exists());
}

#[test]
fn a_file_that_cannot_be_read_reads_as_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::create_dir(&path).unwrap();

    let outcome = ConfigFile::read(&path);

    assert!(matches!(outcome, ReadOutcome::Unreadable));
    assert!(!dir.path().join("config.hcl.invalid").exists());
}

#[test]
fn an_invalid_file_whose_copy_fails_reads_as_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("config.hcl");
    std::fs::write(&path, "zoom = = [\n").unwrap();
    std::fs::create_dir(dir.path().join("config.hcl.invalid")).unwrap();

    let outcome = ConfigFile::read(&path);

    assert!(matches!(outcome, ReadOutcome::Unreadable));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "zoom = = [\n");
}

#[test]
fn write_is_atomic_and_leaves_no_tmp_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("config.hcl");

    ConfigFile::write_atomic(&path, "a = 1\n").unwrap();
    ConfigFile::write_atomic(&path, "a = 2\n").unwrap();

    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a = 2\n");
    let names: Vec<_> = std::fs::read_dir(path.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from("config.hcl")]);
}
