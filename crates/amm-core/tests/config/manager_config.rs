use amm_core::config::{ExclusiveResolution, ManagerConfig, PairKey};
use amm_core::structs::descriptor::Capability;
use curia::Level;

#[test]
fn log_level_for_falls_back_to_global_then_info() {
    let config: ManagerConfig = hcl::from_str(
        r#"
log_level = "Debug"
mod_log_levels = {
  alaydriem_freecam = "Warn"
  alaydriem_odd     = "Loud"
}
"#,
    )
    .unwrap();

    assert_eq!(config.log_level_for("@alaydriem/freecam"), Level::Warn);
    assert_eq!(config.log_level_for("@alaydriem/dejavu"), Level::Debug);
    assert_eq!(config.log_level_for("@alaydriem/odd"), Level::Info);
}

#[test]
fn config_reads_chord_text_and_resolutions_from_hcl() {
    let config: ManagerConfig = hcl::from_str(
        r#"
toggle_chord = "Ctrl+Alt+K"
resolutions {
  exclusive = { Camera__alaydriem_freecam__alaydriem_dejavu = "Prefer:@alaydriem/freecam" }
}
"#,
    )
    .unwrap();

    assert!(config.toggle_chord.ctrl && config.toggle_chord.alt && !config.toggle_chord.shift);
    let key = PairKey::new(Capability::Camera, "@alaydriem/dejavu", "@alaydriem/freecam");
    assert_eq!(config.resolutions.exclusive_for(&key), Some(&ExclusiveResolution::Prefer("@alaydriem/freecam".into())));
}

#[test]
fn empty_config_gives_the_defaults() {
    let config: ManagerConfig = hcl::from_str("").unwrap();

    assert_eq!(config.telemetry_interval_secs, 10);
    assert_eq!(config.log_sessions_kept, 20);
    assert_eq!(config.budgets.max_mods, 32);
    assert!(config.is_enabled("anything"));
}

#[test]
fn enabled_flags_are_read_by_mod_id_from_namespace_underscore_name_keys() {
    let config: ManagerConfig = hcl::from_str(
        r#"
enabled {
  alaydriem_freecam = false
  amml_modmanager   = true
}
"#,
    )
    .unwrap();

    assert!(!config.is_enabled("@alaydriem/freecam"));
    assert!(config.is_enabled("@amml/modmanager"));
    assert!(config.is_enabled("@alaydriem/dejavu"));
}
