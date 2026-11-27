use std::fs;
use std::path::Path;

use amm_abi::types::AmmLogLevel;

use crate::support::{Descriptors, HostFixture, SimMod};

const MANAGER: (&str, &str) = ("amml", "modmanager");

// The one session file of a mod: its text.
fn session_text(dir: &Path) -> String {
    let files: Vec<_> = fs::read_dir(dir)
        .unwrap_or_else(|error| panic!("log dir {}: {error}", dir.display()))
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "log"))
        .collect();
    assert_eq!(files.len(), 1, "one session file in {}", dir.display());
    fs::read_to_string(&files[0]).unwrap_or_else(|error| panic!("log file: {error}"))
}

#[test]
fn the_manager_logs_to_its_own_file_and_names_the_mods_that_registered() {
    let fixture = HostFixture::new();
    let claims = [("Overlay", "Shared")];
    let one = SimMod::new(&fixture, Descriptors::with_claims("sim-one", &claims, ""), "");
    let two = SimMod::new(&fixture, Descriptors::with_claims("sim-two", &claims, ""), "");

    fixture.host_log(AmmLogLevel::Info, "line from the overlay");
    drop(one);
    drop(two);
    fixture.destroy();

    let manager_text = session_text(&fixture.log_dir(MANAGER.0, MANAGER.1));
    assert!(manager_text.contains("line from the overlay"));
    assert!(manager_text.contains("mod registered"));
    assert!(manager_text.contains("sim-one") && manager_text.contains("sim-two"));
}
