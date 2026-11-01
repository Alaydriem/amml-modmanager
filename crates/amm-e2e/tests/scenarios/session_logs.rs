use std::fs;
use std::path::Path;

use amm_abi::types::AmmLogLevel;
use curia::{Fields, Level, LogEvent, Sink};

use crate::support::{Descriptors, HostFixture, SimMod};

const MANAGER: (&str, &str) = ("amml", "modmanager");

fn event(message: &str) -> LogEvent {
    let mut fields = Fields::new();
    fields.insert("origin", "sim");
    LogEvent {
        level: Level::Info,
        target: "sim".to_owned(),
        message: message.to_owned(),
        fields,
        timestamp: chrono::Utc::now(),
        file: None,
        line: None,
    }
}

// The one session file of a mod: its stem and its text.
fn session_file(dir: &Path) -> (String, String) {
    let files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("log dir {}: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
        .collect();
    assert_eq!(files.len(), 1, "one session file in {}", dir.display());
    let stem = files[0].file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default();
    (stem, fs::read_to_string(&files[0]).unwrap_or_else(|error| panic!("log file: {error}")))
}

#[test]
fn each_mod_and_the_manager_log_to_their_own_file_with_one_session_stamp() {
    let fixture = HostFixture::new();
    let claims = [("Overlay", "Shared")];
    let one = SimMod::new(&fixture, Descriptors::with_claims("sim-one", &claims, ""), "");
    let two = SimMod::new(&fixture, Descriptors::with_claims("sim-two", &claims, ""), "");

    one.sink().emit(&event("line from one"));
    two.sink().emit(&event("line from two"));
    fixture.host_log(AmmLogLevel::Info, "line from the overlay");
    drop(one);
    drop(two);
    fixture.destroy();

    let (one_stamp, one_text) = session_file(&fixture.log_dir("test", "sim-one"));
    let (two_stamp, two_text) = session_file(&fixture.log_dir("test", "sim-two"));
    let (manager_stamp, manager_text) = session_file(&fixture.log_dir(MANAGER.0, MANAGER.1));
    assert!(one_text.contains("line from one") && !one_text.contains("line from two"));
    assert!(two_text.contains("line from two") && !two_text.contains("line from one"));
    assert!(manager_text.contains("line from the overlay"));
    assert!(manager_text.contains("mod registered"));
    assert_eq!(one_stamp, two_stamp);
    assert_eq!(one_stamp, manager_stamp);
}
